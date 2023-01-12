//! encode pack file ,and create file
//!
//!
//!
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use bstr::ByteSlice;

use crate::git::errors::GitError;
use crate::git::hash::Hash;
use crate::git::object::diff::DeltaDiff;
use crate::git::object::metadata::MetaData;
use crate::git::object::types::ObjectType;
use crate::git::pack::decode::ObjDecodedMap;
use crate::git::pack::Pack;
use crate::git::utils;

const SLIDING_WINDOW: i32 = 10;

///
/// Pack类的encode函数，将解析出的pack或其他途径生成的pack生成对应的文件
impl Pack {
    /// 对pack文件的头文件进行编码,除了size大小 这部分都是基本固定的 ：
    /// ```plaintext
    ///         -> |'P' 'A' 'C' 'K'  |4b
    /// version -> | 0   0   0   2   |4b
    ///   size  -> | size[ 31 --- 0 ]|4b
    /// ```
    /// Pack对象应先携带有效的 `self.number_of_objects` 字段
    fn encode_header(&mut self) -> Vec<u8> {
        self.head = *b"PACK";
        self.version = 2;
        let mut result: Vec<u8> = vec![
            b'P', b'A', b'C', b'K', // The logotype of the Pack File
            0, 0, 0, 2,
        ]; // THe Version  of the Pack File
        let all_num = self.get_object_number();
        assert!(all_num != 0); // guarantee self.number_of_objects!=0
        assert!(all_num < (1 << 32)); //TODO: GitError:numbers of objects should  < 4G ,
        //Encode the number of object  into file
        result.append(&mut utils::u32_vec(all_num as u32));
        result
    }
    /// 计算pack文件的hash value，赋予id字段，并将hash转为Vec<u8> 输出
    fn append_hash_signature(&mut self, data: &Vec<u8>) -> Vec<u8> {
        let checksum = Hash::new(&data);
        self.signature = checksum.clone();
        checksum.0.to_vec()
    }

    #[allow(unused)]
    /// Pack 结构体的`encode`函数
    ///  > 若输出的meta_vec ==None 则需要pack结构体是完整有效的，或者至少其中的PackObjectCache不为空
    ///  > 若输入的meta_vec不为None 则按照该vec进行encode
    /// # Examples
    /// ```
    ///   let result:Vec<u8> = decoded_pack.encode(None);
    ///     //or
    ///   let metadata_vec :Vec<Metadata> = ...;// Get a list of metadata
    ///   let result:Vec<u8> = Pack::default().encode(metadata_vec);  
    /// ```
    ///
    pub fn encode(&mut self, meta_vec: Option<Vec<MetaData>>) -> Vec<u8> {
        use sha1::{Digest, Sha1};
        let mut result: Vec<u8>;
        let mut offset = 12;
        match meta_vec {
            // 有metadata的情况下
            Some(a) => {
                self.number_of_objects = a.len();
                result = self.encode_header();
                for metadata in a {
                    result.append(&mut metadata.convert_to_vec().unwrap());
                    //self.result.update(Arc::new(metadata), offset);
                    // println!("Decode offset:{}", offset);
                    offset = result.len() as u64;
                }
            }
            None => {
                self.number_of_objects = self.result.by_hash.len();
                result = self.encode_header();
                for (key, value) in self.result.by_hash.iter() {
                    result.append(&mut value.convert_to_vec().unwrap());
                }
            }
        }
        // compute pack hash signature and append to the result
        result.append(&mut self.append_hash_signature(&result));
        result
    }

    /// 仅支持offset delta
    /// 一次通过metadata的完整data输出
    /// 从decode的 `vec_sliding_window` 来
    #[allow(unused)]
    pub fn encode_delta(meta_vec: Vec<MetaData>) -> (Self, Vec<u8>) {
        let mut _pack = Pack::default();
        _pack.number_of_objects = meta_vec.len();
        let mut result = _pack.encode_header();
        let mut code_meta = vec![];
        assert_eq!(result.len(), 12);

        let mut offset: Vec<u64> = vec![]; //记录已完成的metadata的offset

        for i in 0.._pack.number_of_objects as i32 {
            let mut new_meta = meta_vec[i as usize].clone();
            let mut best_j: i32 = 11;
            let mut best_ssam_rate: f64 = 0.0;
            for j in 1..SLIDING_WINDOW {
                if i - j < 0 {
                    break;
                }
                let _base = meta_vec[(i - j) as usize].clone();
                // 若两个对象类型不相同则不进行delta
                if new_meta.t != _base.t {
                    break;
                }
                let diff = DeltaDiff::new(_base.clone(), new_meta.clone());
                let _rate = diff.get_ssam_rate();
                if (_rate > best_ssam_rate) && _rate > 0.5 {
                    best_ssam_rate = _rate;
                    best_j = j;
                }
            }

            let mut final_meta = new_meta.clone();
            if best_j != 11 {
                let _base = meta_vec[(i - best_j) as usize].clone();
                let diff = DeltaDiff::new(_base.clone(), new_meta.clone());
                let zlib_data = diff.get_delta_metadata();
                let offset_head = utils::write_offset_encoding(
                    result.len() as u64 - offset[(i - best_j) as usize],
                );
                final_meta.change_to_delta(ObjectType::OffsetDelta, zlib_data, offset_head);
            }
            code_meta.push(final_meta.clone());
            // TODO:update the offset and write
            offset.push(result.len() as u64);
            result.append(&mut final_meta.convert_to_vec().unwrap());
            println!();
            println!("Hash :{}", final_meta.id);
            println!("type: {}", final_meta.t);
            println!("Offset: {}", offset.last().unwrap());
        }
        let mut _hash = _pack.append_hash_signature(&result);
        result.append(&mut _hash);
        (_pack, result)
    }
    /// Pack the loose object from the Given string .
    /// `obj_path`: the vector of the Hash value of the loose object
    /// `loose_root_path` : loose objects' root path
    /// `target_path` : the pack file store path
    /// 将所有的loose文件读入并写入
    pub fn pack_loose(obj_path: Vec<String>, loose_root_path: &str) -> (Self, Vec<u8>) {
        let mut meta_vec = vec![];
        for path in &obj_path {
            let hash_value = Hash::from_str(path).unwrap();
            let loose_path = format!(
                "{}/{}/{}",
                loose_root_path,
                hash_value.to_folder(),
                hash_value.to_filename()
            );
            let _meta = MetaData::read_object_from_file(loose_path);
            match _meta {
                Ok(meta) => meta_vec.push(meta),
                Err(e) => eprintln!("{}", e),
            }
        }

        // if meta_vec.len() != obj_path.len(){
        //     return false;
        // }
        let mut pack = Pack::default();

        let pack_file_data = pack.encode(Some(meta_vec));
        (pack, pack_file_data)
    }
    /// Pack the loose object from the Given string .
    /// `obj_path`: the vector of the Hash value of the loose object
    /// `loose_root_path` : loose objects' root path
    /// `target_path` : the pack file store path
    ///
    pub fn pack_loose_files(
        obj_path: Vec<String>,
        loose_root_path: &str,
        target_path: &str,
    ) -> Self {
        let (mut _pack, pack_file_data) = Self::pack_loose(obj_path, loose_root_path);
        let pack_file_name = format!(
            "{}/pack-{}.pack",
            target_path,
            _pack.signature.to_plain_str()
        );
        print!("to——file: {}", pack_file_name);
        let mut file = std::fs::File::create(pack_file_name).expect("create failed");
        file.write_all(pack_file_data.as_bytes())
            .expect("write failed");
        _pack
    }
    /// Pack the loose object in a dir ,such as the `.git/object/pack`<br>
    /// It can auto find the loose object follow the position like below:
    /// ```plaintext
    /// ./in：loose_root/aa/bbbbbbbbbbbbbbbbbb
    /// ```
    /// ,The object Hash is `aabbbbbbbbbbbbbbbbbb`
    /// - in：loose_root  : loose object root dir
    /// - in: target_path : The pack file dir to store
    ///
    /// 查找到所有的loose文件代表的Hash值
    pub fn find_all_loose(loose_root_path: &str) -> Vec<String> {
        let loose_root = std::path::PathBuf::from(loose_root_path);
        let mut loose_vec = Vec::new();
        // 打开loose 根目录
        let paths = std::fs::read_dir(&loose_root).unwrap();
        // 暂时保存根目录作为 Path buff
        let mut loose_file = loose_root.clone();
        // loose_file= ./root
        // 遍历目录下的hash前两位(1b)的子文件夹
        for path in paths {
            if let Ok(hash_2) = path {
                //the first 1 b
                let file_name1 = String::from(hash_2.file_name().to_str().unwrap());

                // 判断只有两位且是文件夹
                let is_dir = hash_2.file_type().unwrap().is_dir();
                if is_dir && (file_name1.len() == 2) {
                    loose_file.push(file_name1.clone());
                    //loose_file = ./root/xx
                    let loose_s = std::fs::read_dir(&loose_file).unwrap();

                    //再打开子文件夹 此目录下即为保存的loose object文件
                    for loose_path in loose_s {
                        if let Ok(loose_path) = loose_path {
                            let file_name2 = String::from(loose_path.file_name().to_str().unwrap());
                            loose_file.push(file_name2.clone());
                            //loose_file = ./root/xx/xxxxxxxxxxxxxxxxxxxx
                            //将object提取hash值并放入vec
                            loose_vec.push(
                                Hash::from_str(&(file_name1.clone() + &file_name2))
                                    .unwrap()
                                    .to_plain_str(),
                            );
                            loose_file.pop(); // pop path buf
                        }
                    }
                    loose_file.pop();
                } else {
                    continue;
                }
            }
        }

        loose_vec
    }
    /// 从文件夹中将所有loose文件压缩
    #[allow(unused)]
    pub fn pack_loose_from_dir(loose_root_path: &str, target_path: &str) -> Self {
        let loose_vec = Self::find_all_loose(loose_root_path);
        Pack::pack_loose_files(loose_vec, loose_root_path, target_path)
    }

    /// 找到pack文件 //TODO: 目前只支持单个文件 ,之后将考虑多文件
    fn find_pack_file(object_dir: &str) -> File {
        let mut object_root = std::path::PathBuf::from(object_dir);
        let mut pack_file_name = String::new();
        object_root.push("pack");
        let paths = std::fs::read_dir(&object_root).unwrap();
        for path in paths {
            if let Ok(pack_file) = path {
                let _file_name = pack_file.file_name();
                let _file_name = _file_name.to_str().unwrap();
                if &_file_name[_file_name.len() - 4..] == "pack" {
                    pack_file_name.push_str(_file_name);
                    break;
                }
            }
        }
        object_root.push(pack_file_name);

        let pack_file = File::open(object_root).unwrap();
        pack_file
    }
    #[allow(unused)]
    pub fn pack_object_dir(object_dir: &str, target_dir: &str) -> Self {
        // unpack the pack file which should be unchanged
        let mut pack_file = Self::find_pack_file(object_dir);
        let (raw_pack, mut raw_data) = Pack::decode_raw_data(&mut pack_file);
        // 将loose object 预先压缩
        let loose_vec = Self::find_all_loose(object_dir);
        let (mut loose_pack, loose_data) = Pack::pack_loose(loose_vec, object_dir);

        // 创建新的pack对象
        let mut new_pack = Self::default();
        new_pack.head = *b"PACK";
        new_pack.version = 2;
        new_pack.number_of_objects = raw_pack.get_object_number() + loose_pack.get_object_number();
        let mut result = new_pack.encode_header();

        result.append(&mut raw_data);
        let mut loose_data = utils::get_pack_raw_data(loose_data);
        result.append(&mut loose_data);
        new_pack.signature = Hash::new(&result);
        result.append(&mut new_pack.signature.0.to_vec());

        // 开始写入
        let mut file = std::fs::File::create(format!(
            "{}/pack-{}.pack",
            target_dir,
            new_pack.signature.to_plain_str()
        ))
            .expect("create failed");
        file.write_all(result.as_bytes()).expect("write failed");

        new_pack
    }
    #[allow(unused)]
    pub fn write(map: &mut ObjDecodedMap, target_dir: &str) -> Result<(), GitError> {
        map.check_completeness().unwrap();
        let meta_vec = map.vec_sliding_window();
        let (_pack, data_write) = Pack::encode_delta(meta_vec);
        let mut to_path = PathBuf::from(target_dir);
        let file_name = format!("pack-{}.pack", _pack.signature.to_plain_str());
        to_path.push(file_name);
        let mut file = std::fs::File::create(to_path).expect("create failed");
        file.write_all(data_write.as_bytes()).expect("write failed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use bstr::ByteSlice;

    use crate::git::pack::{decode::ObjDecodedMap, Pack};

    const TEST_DIR: &str = "./test_dir";

    #[test]
    fn test_object_dir_encode() {
        Pack::pack_object_dir("./resources/total", "./resources/total/output");
        let decoded_pack = Pack::decode_file(
            "./resources/total/output/pack-7ea8ad41c9d438654ef28297ecc874842c7d10de.pack",
        );
        println!("{}", decoded_pack.get_object_number());
        assert_eq!(
            "7ea8ad41c9d438654ef28297ecc874842c7d10de",
            decoded_pack.signature.to_plain_str()
        );
    }

    //
    #[test]
    fn test_a_real_pack_de_en() {
        let decoded_pack = Pack::decode_file(
            "./resources/test2/pack-8c81e90db37ef77494efe4f31daddad8b494e099.pack",
        );
        let mut map = ObjDecodedMap::default();
        map.update_from_cache(&decoded_pack.get_cache());
        Pack::write(&mut map, TEST_DIR).unwrap();

        Pack::decode_file("./test_dir/pack-83df56e42ca705892f7fd64f96ecb9870b5c5ed8.pack");
    }

    #[test]
    fn test_multi_pack_encode() {
        let pack_1 = Pack::decode_file(
            "./resources/test1/pack-1d0e6c14760c956c173ede71cb28f33d921e232f.pack",
        );
        let pack_2 = Pack::decode_file(
            "./resources/test2/pack-8c81e90db37ef77494efe4f31daddad8b494e099.pack",
        );

        let mut map = ObjDecodedMap::default();
        map.update_from_cache(&pack_1.get_cache());
        map.update_from_cache(&pack_2.get_cache());

        Pack::write(&mut map, TEST_DIR).unwrap();

        Pack::decode_file("./test_dir/pack-8e8b79ea20effb78d701fa8ad5a7e386b7d833fa.pack");
    }

    #[test]
    fn dex_number() {
        let all_num: usize = 0x100f1109;
        println!("{:x}", (all_num >> 24) as u8);
        println!("{:x}", (all_num >> 16) as u8);
        println!("{:x}", (all_num >> 8) as u8);
        println!("{:x}", (all_num) as u8);
    }

    /// 将一些loose object打包为 pack文件
    /// 只需要给出loose文件的根目录 目标根目录 和 loose 文件的hash字符串即可
    #[test]
    fn test_loose_pack() {
        let mut loose_vec = Vec::new();
        loose_vec.push(format!("5f413c76a2893bb1ff83d7c2b507a9cab30bd585"));
        loose_vec.push(format!("8bb783eb532d4936248f9084821af2bb309f29e7"));
        loose_vec.push(format!("79dc1608dba888e0378ff21591dc646c8afe4e0a"));
        loose_vec.push(format!("ce70a618efa88992a4c4bdf22ebd832b24acf374"));
        let loose_root = "./resources/loose";
        let target_path = "./resources/pack_g";
        let pack = Pack::pack_loose_files(loose_vec, loose_root, target_path);
        Pack::decode_file(&format!(
            "{}/pack-{}.pack",
            target_path,
            pack.signature.to_plain_str()
        ));
    }

    /// 只需要给定loose 的根目录 则自动读取所有loose的文件并打包至指定文件夹
    #[test]
    fn test_loose_pack_from_dir() {
        let loose_root = "./resources/loose";
        let target_path = "./resources/pack_g";
        // 解析过程
        let pack = Pack::pack_loose_from_dir(loose_root, target_path);
        Pack::decode_file(&format!(
            "{}/pack-{}.pack",
            target_path,
            pack.signature.to_plain_str()
        ));
    }

    #[test]
    fn test_delta_pack_ok() {
        let mut _map = ObjDecodedMap::default();
        let decoded_pack = Pack::decode_file(
            "./resources/data/test/pack-6590ba86f4e863e1c2c985b046e1d2f1a78a0089.pack",
        );
        assert_eq!(
            "6590ba86f4e863e1c2c985b046e1d2f1a78a0089",
            decoded_pack.signature.to_plain_str()
        );
        let mut result = ObjDecodedMap::default();
        result.update_from_cache(&decoded_pack.result);
        result.check_completeness().unwrap();
        let meta_vec = result.vec_sliding_window();
        let (_pack, data_write) = Pack::encode_delta(meta_vec);

        let file_name = format!("pack-{}.pack", _pack.signature.to_plain_str());
        let mut file = std::fs::File::create(file_name).expect("create failed");
        file.write_all(data_write.as_bytes()).expect("write failed");

        let decoded_pack =
            Pack::decode_file(&format!("pack-{}.pack", _pack.signature.to_plain_str()));
        assert_eq!(
            "aa2ab2eb4e6b37daf6dcadf1b6f0d8520c14dc89",
            decoded_pack.signature.to_plain_str()
        );
    }

    // #[test]
    // fn test_vec(){
    //     let mut arr = vec! [1,2,3,4,5];
    //     let ta = arr.last_mut().unwrap();
    //     *ta += 8;
    //     print!("{:?}",arr);
    // }
}
