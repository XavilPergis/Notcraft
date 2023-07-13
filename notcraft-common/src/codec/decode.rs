// fn read_one_byte<R: Read>(reader: &mut R) -> Result<u8> {
//     let mut buf = [0];
//     reader.read_exact(&mut buf)?;
//     Ok(buf[0])
// }

// fn decode_u8<R: Read>(reader: &mut R) -> Result<u8> {
//     let mut cur = 0;
//     let mut shift = 0;

//     loop {
//         let octet = read_one_byte(reader)?;
//         cur |= ((octet & 0x7f) as u8) << shift;
//         shift += 7;

//         if octet & 0x80 == 0 {
//             break;
//         }
//     }

//     Ok(cur)
// }

// fn decode_i8<R: Read>(reader: &mut R) -> Result<i8> {
//     let mut cur = 0;
//     let mut shift = 0;

//     loop {
//         let octet = read_one_byte(reader)?;
//         match octet & 0x80 != 0 {
//             true => {
//                 cur |= ((octet & 0x7f) as i8) << shift;
//                 shift += 7;
//             }
//             false => {
//                 let sign = octet & 0x40 != 0;
//                 cur |= ((octet & 0x3f) as i8) << shift;
//                 cur *= [1, -1][sign as usize];
//                 break;
//             }
//         }
//     }

//     Ok(cur)
// }
