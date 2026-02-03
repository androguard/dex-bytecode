use crate::decoder::Decoder;

#[test]
fn decoder_try_with_ip() {
    let decoder = Decoder::try_with_ip(b"", 0x1234_5678_9ABC_DEF1, 0).unwrap();
    assert_eq!(decoder.ip(), 0x1234_5678_9ABC_DEF1);
}
