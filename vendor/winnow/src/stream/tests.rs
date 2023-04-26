#[cfg(feature = "std")]
use proptest::prelude::*;

use super::*;

#[test]
fn test_offset_u8() {
    let s = b"abcd123";
    let a = &s[..];
    let b = &a[2..];
    let c = &a[..4];
    let d = &a[3..5];
    assert_eq!(a.offset_to(b), 2);
    assert_eq!(a.offset_to(c), 0);
    assert_eq!(a.offset_to(d), 3);
}

#[test]
fn test_offset_str() {
    let a = "abcřèÂßÇd123";
    let b = &a[7..];
    let c = &a[..5];
    let d = &a[5..9];
    assert_eq!(a.offset_to(b), 7);
    assert_eq!(a.offset_to(c), 0);
    assert_eq!(a.offset_to(d), 5);
}

#[test]
#[cfg(feature = "alloc")]
fn test_bit_stream_empty() {
    let i = (&b""[..], 0);

    let actual = i.iter_offsets().collect::<crate::lib::std::vec::Vec<_>>();
    assert_eq!(actual, vec![]);

    let actual = i.eof_offset();
    assert_eq!(actual, 0);

    let actual = i.next_token();
    assert_eq!(actual, None);

    let actual = i.offset_for(|b| b);
    assert_eq!(actual, None);

    let actual = i.offset_at(1);
    assert_eq!(actual, Err(Needed::new(1)));

    let (actual_input, actual_slice) = i.next_slice(0);
    assert_eq!(actual_input, (&b""[..], 0));
    assert_eq!(actual_slice, (&b""[..], 0, 0));
}

#[test]
#[cfg(feature = "alloc")]
fn test_bit_offset_empty() {
    let i = (&b""[..], 0);

    let actual = i.offset_to(&i);
    assert_eq!(actual, 0);
}

#[cfg(feature = "std")]
proptest! {
  #[test]
  #[cfg_attr(miri, ignore)]  // See https://github.com/AltSysrq/proptest/issues/253
  fn bit_stream(byte_len in 0..20usize, start in 0..160usize) {
        bit_stream_inner(byte_len, start);
  }
}

#[cfg(feature = "std")]
fn bit_stream_inner(byte_len: usize, start: usize) {
    let start = start.min(byte_len * 8);
    let start_byte = start / 8;
    let start_bit = start % 8;

    let bytes = vec![0b1010_1010; byte_len];
    let i = (&bytes[start_byte..], start_bit);

    let mut curr_i = i;
    let mut curr_offset = 0;
    while let Some((next_i, _token)) = curr_i.next_token() {
        let to_offset = i.offset_to(&curr_i);
        assert_eq!(curr_offset, to_offset);

        let (slice_i, _) = i.next_slice(curr_offset);
        assert_eq!(curr_i, slice_i);

        let at_offset = i.offset_at(curr_offset).unwrap();
        assert_eq!(curr_offset, at_offset);

        let eof_offset = curr_i.eof_offset();
        let (next_eof_i, eof_slice) = curr_i.next_slice(eof_offset);
        assert_eq!(next_eof_i, (&b""[..], 0));
        let eof_slice_i = (eof_slice.0, eof_slice.1);
        assert_eq!(eof_slice_i, curr_i);

        curr_offset += 1;
        curr_i = next_i;
    }
    assert_eq!(i.eof_offset(), curr_offset);
}

#[test]
fn test_partial_complete() {
    let mut i = Partial::new(&b""[..]);
    assert!(Partial::<&[u8]>::is_partial_supported());

    assert!(i.is_partial(), "incomplete by default");
    let incomplete_state = i.complete();
    assert!(!i.is_partial(), "the stream should be marked as complete");

    i.restore_partial(incomplete_state);
    assert!(i.is_partial(), "incomplete stream state should be restored");
}
