use hyper::rt::ReadBuf;
use std::io::Read;

const TEXT: &[u8] = r#"Philosophers often behave like little children who scribble some marks on a piece of paper at random and then ask the grown-up "What's that?" — It happened like this: the grown-up had drawn pictures for the child several times and said "this is a man," "this is a house," etc. And then the child makes some marks too and asks: what's this then?"#.as_bytes();

#[test]
fn with_read() {
    // Create the buffer.
    let mut buffer = [0u8; TEXT.len() + 27];
    let mut read_buf = ReadBuf::new(&mut buffer);
    let mut cursor = read_buf.unfilled();

    // Read into it.
    cursor.read_with(|out| {
        let mut text = TEXT;
        text.read(out)
    }).unwrap();

    assert_eq!(read_buf.filled(), TEXT);
    assert_eq!(&buffer[..TEXT.len()], TEXT);
}
