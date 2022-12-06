//!Idx file , which is in the dir:`.git/object/pack/*.idx`
//!
//!This file provides the offset of different objects, 
//! which is used to quickly find the target object in the pack file(*.pack).
//! 


use std::collections::HashMap;
use std::fmt::Display;
use std::io::{Cursor, Write};

use byteorder::{BigEndian, ReadBytesExt};
use deflate::Compression;
use deflate::write::ZlibEncoder;
use crate::errors::GitError;
use crate::git::hash::Hash;
use crate::git::id::ID;
use crate::utils;

use super::pack::Pack;

///
#[allow(unused)]
pub struct IdxItem {
    pub id: ID,
    pub crc32: String,
    pub offset: usize,
}

///
impl Display for IdxItem {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{} {} ({})", self.offset, self.id, self.crc32)
    }
}

///
#[allow(unused)]
#[derive(Default)]
pub struct Idx {
    pub version: u32,
    pub number_of_objects: usize,
    pub map_of_prefix: HashMap<String, usize>,
    pub idx_items: Vec<IdxItem>,
    pub pack_signature: ID,
    pub idx_signature: ID,
    _file_data:Vec<u8>,
}

///
impl Idx {
    ///
    #[allow(unused)]
    fn sha1_prefix(&self, n: usize) -> String {
        let pre = format!("{:x}", n);

        if pre.len() == 1 {
            format!("0{}", pre)
        } else {
            pre
        }
    }

    ///
    #[allow(unused)]
    pub fn decode(& mut self, data: Vec<u8>) -> Result<(), GitError> {
        let mut offset : usize = 0;

        let mut id_of_objects: Vec<ID> = Vec::new();
        let mut crc32_of_objects: Vec<String> = Vec::new();

        // 4-byte Header: //FF 74 4F 63
        if data[offset..4].to_vec() != vec![255, 116, 79, 99] {
            return Err(GitError::InvalidIdxFile(format!("Invalid idx header: {:?}", data[0..4].to_vec())));
        }
        offset += 4;

        // 4-byte version number (network byte order):
        let mut v = Cursor::new(data[offset..8].to_vec());
        self.version = v.read_u32::<BigEndian>().unwrap();
        offset += 4;

        // Layer 1:
        //  Number of objects in the pack (network byte order)
        //  The prefix of the SHA-1 hash of the object has how many objects it is in the pack.
        let mut n : usize = 0;
        for i in (offset..offset + 256 * 4).filter(|x| ((x - offset) % 4 == 0)) {
            let mut v = Cursor::new(data[i..i + 4].to_vec());
            let m = v.read_u32::<BigEndian>().unwrap() as usize;

            if m != n {
                self.map_of_prefix.insert(self.sha1_prefix((i - 8)/4), m - n);
                self.number_of_objects = m;
                n = m;
            }
        }
        offset += 256 * 4; // 1040

        // Layer 2:
        //  The all the SHA-1 hashes of the objects in the pack.
        for i in (offset..offset + (20 * n) as usize).filter(|x| ((x - offset) % 20 == 0))  {
            let id = ID::from_bytes(&data[(i as usize)..(i as usize) + 20]);
            id_of_objects.push(id);
        }
        offset += 20 * n as usize;


        // Layer 3:
        //   The CRC32 of the object data.
        for i in (offset..offset + (4 * n) as usize).filter(|x| ((x - offset) % 4 == 0)) {
            crc32_of_objects.push(hex::encode(&data[i..i + 4]));
        }
        offset += 4 * n as usize;


        // Layer 4:
        //   the object offset in the pack file.
        let mut index = 0;
        for (index, i) in (offset..offset + (4 * n) as usize).filter(|x| ((x - offset) % 4 == 0)).enumerate() {
            let mut v = Cursor::new(data[i..i + 4].to_vec());
            let m = v.read_u32::<BigEndian>().unwrap() as usize;

            self.idx_items.push(IdxItem {
                id: id_of_objects[index].clone(),
                crc32: crc32_of_objects[index].clone(),
                offset: m,
            });
        }
        offset += 4 * n as usize;

        // Layer 5

        // Layer 6:
        //  The SHA-1 hash of the pack file itself.
        //  The SHA-1 hash of the index file itself.
        self.pack_signature = ID::from_bytes(&data[offset..offset + 20]);
        offset += 20;
        self.idx_signature = ID::from_bytes(&data[offset..]);

        Ok(())
    }

    #[allow(unused)]
    pub fn encode(pack:Pack) -> Self{
        let mut idx = Self::default();
        let mut result:Vec<u8>  =  vec![255, 116, 79, 99];//header
        let mut version:Vec<u8> = vec![0,0,0,2];
        result.append(&mut version);
        idx.version = 2;

        // Layer 1:
        //  Number of objects in the pack (network byte order)
        //  The prefix of the SHA-1 hash of the object has how many objects it is in the pack.
        idx.number_of_objects = pack.get_object_number();
        let mut  fan_out :[u32;256] = [0;256];
        let cache = pack.get_cache();
        for (key,value) in cache.by_hash.iter() {
            fan_out[key.get_first() as usize] +=1;
        }
        let mut _sum = 0;
        for i in 0..256 {
            _sum +=fan_out[i];
            fan_out[i] = _sum;
            result.append(&mut utils::u32_vec(fan_out[i])); 
        }

        // Layer 2:
        //  The all the SHA-1 hashes of the objects in the pack.
        for key in cache.by_hash.keys() {
            result.append(&mut key.0.to_vec())
        }

       
        // Layer 3: 
        //   The CRC32 of the object data.
            //BUG: The Algorithm of the crc32 is different from the official git, 
            // and maybe the compress data is not same between the different storage type
            // So this crc32 computing is different from the git crc32.
            // But cause we haven't do the crc32 check , so That's Ok ,
            // Other code still can parse objects by the idx and pack file correctly

        // NO.1 try code seg。crc32编码的尝试代码
        use crc::{Crc, Algorithm, CRC_32_ISO_HDLC};
        pub const CASTAGNOLI: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);
        for values in cache.by_hash.values() {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::Default);
            encoder.write_all(&values.contents[..]).expect("Write error!");
            let zlib_data =   encoder.finish().expect("Failed to finish compression!");
            result.append(&mut utils::u32_vec(CASTAGNOLI.checksum(&zlib_data))); 
        }
            // NO.2 try code seg 。crc32编码的尝试代码
            // for values in cache.by_hash.values() {
            //    let mut crc32s = hex::decode(values.contents.clone()).unwrap();
            //    result.append(&mut crc32s);
            // }

        // Layer 4:
        //   the object offset in the pack file.
        for offset in cache.by_offset.values(){
            result.append(&mut utils::u32_vec( *offset as u32));
        }
        
        // Layer 5 only for the big offset > 4G , temporary skip

        // Layer 6:
        //  The SHA-1 hash of the pack file itself.
        let pack_hash = pack.get_hash();
        result.append(&mut pack_hash.0.to_vec());
        //  The SHA-1 hash of the index file itself.
        let idx_hash = Hash::new(&result) ;
        result.append(&mut idx_hash.0.to_vec());
        idx._file_data = result;
        idx
    }
}

///
#[cfg(test)]
mod tests {
    use std::env;
    use std::fs::File;
    use std::io::{BufReader,Read,Write};
    use std::path::{Path,PathBuf};
    use bstr::ByteSlice;
    use crate::utils;
    use super::Idx;
    ///测试读取idx
    #[test]
    fn test_idx_read_from_file() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("resources/data/test/pack-8d36a6464e1f284e5e9d06683689ee751d4b2687.idx");

        let f = File::open(path).ok();
        let mut reader = BufReader::new(f.unwrap());
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).ok();
        let mut idx = Idx::default();
        idx.decode(buffer).unwrap();

        assert_eq!(2, idx.version);
        assert_eq!(614, idx.number_of_objects);
        assert_eq!(2, idx.map_of_prefix["7c"]);
        assert_eq!(idx.number_of_objects, idx.idx_items.len());
        assert_eq!("8d36a6464e1f284e5e9d06683689ee751d4b2687", idx.pack_signature.to_string());
        assert_eq!("92d07408a070a5fbea3c1f2d00e696293b78e7c6", idx.idx_signature.to_string());
    }

    ///测试写入idx文件
    #[test]
    fn test_idx_write_to_file() {
        
          // "./resources/data/test/pack-6590ba86f4e863e1c2c985b046e1d2f1a78a0089.pack"
       use super::super::pack;
       let  packs = pack::Pack::decode_file(
       "./resources/test1/pack-1d0e6c14760c956c173ede71cb28f33d921e232f.pack" 
       );
       let idx = Idx::encode(packs);

       let mut file = std::fs::File::create("./test.idx").expect("create failed");
       file.write_all(idx._file_data.as_bytes()).expect("write failed");

       println!("data written to file");
       let  idx_file = File::open(&Path::new("./test.idx")).unwrap();

       let mut reader = BufReader::new(idx_file);
       let mut buffer = Vec::new();
       reader.read_to_end(&mut buffer).ok();

        let mut idx = Idx::default();

        idx.decode(buffer).unwrap();


    }

    /// fan out table create test
    #[test]
    fn unsafe_fan_out(){
        let mut result :Vec<u8>= vec![];
        let mut  fan_out :[u32;256] = [0;256];
        let mut _sum  = 0;
        for i in 0..255 {
            _sum +=fan_out[i]+5;
            fan_out[i] = _sum;
            result.append(&mut utils::u32_vec(fan_out[i])); 
        }
        assert_eq!(result[0..4],[0,0,0,5]);
        assert_eq!(result[4..8],[0,0,0,10]);
    }

    // crc32 create test
    #[test]
    fn test_crc32(){
        use crc::{Crc, CRC_32_ISCSI};
        pub const CASTAGNOLI: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);
        assert_eq!(CASTAGNOLI.checksum(b"123456789"), 0xe3069283);

    }

}