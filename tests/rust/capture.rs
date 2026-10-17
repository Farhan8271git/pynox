use super::*;

#[test]
fn temporary_lack_of_output_is_not_eof() {
    let (reader, mut writer) = pipe().unwrap();
    let mut stream = Stream::new(reader, 16).unwrap();
    stream.read_available().unwrap();
    assert!(stream.data.is_empty());
    assert!(!stream.eof);
    writer.write_all(b"later").unwrap();
    drop(writer);
    stream.read_available().unwrap();
    assert_eq!(stream.data, b"later");
    assert!(stream.eof);
    assert!(!stream.truncated);
}

#[test]
fn output_is_drained_after_the_capture_limit() {
    let (reader, mut writer) = pipe().unwrap();
    let mut stream = Stream::new(reader, 5).unwrap();
    writer.write_all(b"hello").unwrap();
    stream.read_available().unwrap();
    assert_eq!(stream.data, b"hello");
    assert!(!stream.truncated);
    // Repeated small writes avoid depending on the host's pipe capacity.
    for _ in 0..32 {
        writer.write_all(&[b'x'; 1024]).unwrap();
        stream.read_available().unwrap();
    }
    drop(writer);
    stream.read_available().unwrap();
    assert_eq!(stream.data, b"hello");
    assert!(stream.truncated);
    assert!(stream.eof);
}

#[test]
fn exact_limit_preserves_raw_bytes_without_marking_truncation() {
    let (reader, mut writer) = pipe().unwrap();
    let bytes = [b'a', 0xff, 0, b'z'];
    writer.write_all(&bytes).unwrap();
    drop(writer);
    let mut stream = Stream::new(reader, bytes.len()).unwrap();
    stream.read_available().unwrap();
    assert_eq!(stream.data, bytes);
    assert!(!stream.truncated);
    assert!(stream.eof);
}

#[test]
fn capture_descriptors_are_closed_on_exec() {
    let (reader, writer) = pipe().unwrap();
    for file in [&reader, &writer] {
        let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) };
        assert!(flags >= 0);
        assert_ne!(flags & libc::FD_CLOEXEC, 0);
    }
}
