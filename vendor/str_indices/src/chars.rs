//! Index by chars.

use crate::byte_chunk::{ByteChunk, Chunk};

/// Counts the chars in a string slice.
///
/// Runs in O(N) time.
#[inline]
pub fn count(text: &str) -> usize {
    count_impl::<Chunk>(text.as_bytes())
}

/// Converts from byte-index to char-index in a string slice.
///
/// If the byte is in the middle of a multi-byte char, returns the index of
/// the char that the byte belongs to.
///
/// Any past-the-end index will return the one-past-the-end char index.
///
/// Runs in O(N) time.
#[inline]
pub fn from_byte_idx(text: &str, byte_idx: usize) -> usize {
    let bytes = text.as_bytes();

    // Ensure the index is either a char boundary or is off the end of
    // the text.
    let mut i = byte_idx;
    while Some(true) == bytes.get(i).map(|byte| (*byte & 0xC0) == 0x80) {
        i -= 1;
    }

    count_impl::<Chunk>(&bytes[0..i.min(bytes.len())])
}

/// Converts from char-index to byte-index in a string slice.
///
/// Any past-the-end index will return the one-past-the-end byte index.
///
/// Runs in O(N) time.
#[inline]
pub fn to_byte_idx(text: &str, char_idx: usize) -> usize {
    to_byte_idx_impl::<Chunk>(text, char_idx)
}

//-------------------------------------------------------------

#[inline(always)]
fn to_byte_idx_impl<T: ByteChunk>(text: &str, char_idx: usize) -> usize {
    // Get `middle` so we can do more efficient chunk-based counting.
    // We can't use this to get `end`, however, because the start index of
    // `end` actually depends on the accumulating char counts during the
    // counting process.
    let (start, middle, _) = unsafe { text.as_bytes().align_to::<T>() };

    let mut byte_count = 0;
    let mut char_count = 0;

    // Take care of any unaligned bytes at the beginning.
    for byte in start.iter() {
        char_count += ((*byte & 0xC0) != 0x80) as usize;
        if char_count > char_idx {
            break;
        }
        byte_count += 1;
    }

    // Process chunks in the fast path.
    let mut chunks = middle;
    let mut max_round_len = char_idx.saturating_sub(char_count) / T::MAX_ACC;
    while max_round_len > 0 && !chunks.is_empty() {
        // Choose the largest number of chunks we can do this round
        // that will neither overflow `max_acc` nor blast past the
        // remaining line breaks we're looking for.
        let round_len = T::MAX_ACC.min(max_round_len).min(chunks.len());
        max_round_len -= round_len;
        let round = &chunks[..round_len];
        chunks = &chunks[round_len..];

        // Process the chunks in this round.
        let mut acc = T::zero();
        for chunk in round.iter() {
            acc = acc.add(chunk.bitand(T::splat(0xc0)).cmp_eq_byte(0x80));
        }
        char_count += (T::SIZE * round_len) - acc.sum_bytes();
        byte_count += T::SIZE * round_len;
    }

    // Process chunks in the slow path.
    for chunk in chunks.iter() {
        let new_char_count =
            char_count + T::SIZE - chunk.bitand(T::splat(0xc0)).cmp_eq_byte(0x80).sum_bytes();
        if new_char_count >= char_idx {
            break;
        }
        char_count = new_char_count;
        byte_count += T::SIZE;
    }

    // Take care of any unaligned bytes at the end.
    let end = &text.as_bytes()[byte_count..];
    for byte in end.iter() {
        char_count += ((*byte & 0xC0) != 0x80) as usize;
        if char_count > char_idx {
            break;
        }
        byte_count += 1;
    }

    byte_count
}

#[inline(always)]
pub(crate) fn count_impl<T: ByteChunk>(text: &[u8]) -> usize {
    if text.len() < T::SIZE {
        // Bypass the more complex routine for short strings, where the
        // complexity hurts performance.
        text.iter()
            .map(|byte| ((byte & 0xC0) != 0x80) as usize)
            .sum()
    } else {
        // Get `middle` for more efficient chunk-based counting.
        let (start, middle, end) = unsafe { text.align_to::<T>() };

        let mut inv_count = 0;

        // Take care of unaligned bytes at the beginning.
        for byte in start.iter() {
            inv_count += ((byte & 0xC0) == 0x80) as usize;
        }

        // Take care of the middle bytes in big chunks.
        for chunks in middle.chunks(T::MAX_ACC) {
            let mut acc = T::zero();
            for chunk in chunks.iter() {
                acc = acc.add(chunk.bitand(T::splat(0xc0)).cmp_eq_byte(0x80));
            }
            inv_count += acc.sum_bytes();
        }

        // Take care of unaligned bytes at the end.
        for byte in end.iter() {
            inv_count += ((byte & 0xC0) == 0x80) as usize;
        }

        text.len() - inv_count
    }
}

//=============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // 124 bytes, 100 chars, 4 lines
    const TEXT_LINES: &str = "Hello there!  How're you doing?\nIt's \
                              a fine day, isn't it?\nAren't you glad \
                              we're alive?\nこんにちは、みんなさん！";

    #[test]
    fn count_01() {
        let text = "Hello せかい! Hello せかい! Hello せかい! Hello せかい! Hello せかい!";

        assert_eq!(54, count(text));
    }

    #[test]
    fn count_02() {
        assert_eq!(100, count(TEXT_LINES));
    }

    #[test]
    fn from_byte_idx_01() {
        let text = "Hello せかい!";
        assert_eq!(0, from_byte_idx(text, 0));
        assert_eq!(1, from_byte_idx(text, 1));
        assert_eq!(6, from_byte_idx(text, 6));
        assert_eq!(6, from_byte_idx(text, 7));
        assert_eq!(6, from_byte_idx(text, 8));
        assert_eq!(7, from_byte_idx(text, 9));
        assert_eq!(7, from_byte_idx(text, 10));
        assert_eq!(7, from_byte_idx(text, 11));
        assert_eq!(8, from_byte_idx(text, 12));
        assert_eq!(8, from_byte_idx(text, 13));
        assert_eq!(8, from_byte_idx(text, 14));
        assert_eq!(9, from_byte_idx(text, 15));
        assert_eq!(10, from_byte_idx(text, 16));
        assert_eq!(10, from_byte_idx(text, 17));
        assert_eq!(10, from_byte_idx(text, 18));
        assert_eq!(10, from_byte_idx(text, 19));
    }

    #[test]
    fn from_byte_idx_02() {
        let text = "";
        assert_eq!(0, from_byte_idx(text, 0));
        assert_eq!(0, from_byte_idx(text, 1));

        let text = "h";
        assert_eq!(0, from_byte_idx(text, 0));
        assert_eq!(1, from_byte_idx(text, 1));
        assert_eq!(1, from_byte_idx(text, 2));

        let text = "hi";
        assert_eq!(0, from_byte_idx(text, 0));
        assert_eq!(1, from_byte_idx(text, 1));
        assert_eq!(2, from_byte_idx(text, 2));
        assert_eq!(2, from_byte_idx(text, 3));
    }

    #[test]
    fn from_byte_idx_03() {
        let text = "せかい";
        assert_eq!(0, from_byte_idx(text, 0));
        assert_eq!(0, from_byte_idx(text, 1));
        assert_eq!(0, from_byte_idx(text, 2));
        assert_eq!(1, from_byte_idx(text, 3));
        assert_eq!(1, from_byte_idx(text, 4));
        assert_eq!(1, from_byte_idx(text, 5));
        assert_eq!(2, from_byte_idx(text, 6));
        assert_eq!(2, from_byte_idx(text, 7));
        assert_eq!(2, from_byte_idx(text, 8));
        assert_eq!(3, from_byte_idx(text, 9));
        assert_eq!(3, from_byte_idx(text, 10));
        assert_eq!(3, from_byte_idx(text, 11));
        assert_eq!(3, from_byte_idx(text, 12));
    }

    #[test]
    fn from_byte_idx_04() {
        // Ascii range
        for i in 0..88 {
            assert_eq!(i, from_byte_idx(TEXT_LINES, i));
        }

        // Hiragana characters
        for i in 88..125 {
            assert_eq!(88 + ((i - 88) / 3), from_byte_idx(TEXT_LINES, i));
        }

        // Past the end
        for i in 125..130 {
            assert_eq!(100, from_byte_idx(TEXT_LINES, i));
        }
    }

    #[test]
    fn to_byte_idx_01() {
        let text = "Hello せかい!";
        assert_eq!(0, to_byte_idx(text, 0));
        assert_eq!(1, to_byte_idx(text, 1));
        assert_eq!(2, to_byte_idx(text, 2));
        assert_eq!(5, to_byte_idx(text, 5));
        assert_eq!(6, to_byte_idx(text, 6));
        assert_eq!(12, to_byte_idx(text, 8));
        assert_eq!(15, to_byte_idx(text, 9));
        assert_eq!(16, to_byte_idx(text, 10));
    }

    #[test]
    fn to_byte_idx_02() {
        let text = "せかい";
        assert_eq!(0, to_byte_idx(text, 0));
        assert_eq!(3, to_byte_idx(text, 1));
        assert_eq!(6, to_byte_idx(text, 2));
        assert_eq!(9, to_byte_idx(text, 3));
    }

    #[test]
    fn to_byte_idx_03() {
        let text = "Hello world!";
        assert_eq!(0, to_byte_idx(text, 0));
        assert_eq!(1, to_byte_idx(text, 1));
        assert_eq!(8, to_byte_idx(text, 8));
        assert_eq!(11, to_byte_idx(text, 11));
        assert_eq!(12, to_byte_idx(text, 12));
    }

    #[test]
    fn to_byte_idx_04() {
        let text = "Hello world! Hello せかい! Hello world! Hello せかい! \
                    Hello world! Hello せかい! Hello world! Hello せかい! \
                    Hello world! Hello せかい! Hello world! Hello せかい! \
                    Hello world! Hello せかい! Hello world! Hello せかい!";
        assert_eq!(0, to_byte_idx(text, 0));
        assert_eq!(30, to_byte_idx(text, 24));
        assert_eq!(60, to_byte_idx(text, 48));
        assert_eq!(90, to_byte_idx(text, 72));
        assert_eq!(115, to_byte_idx(text, 93));
        assert_eq!(120, to_byte_idx(text, 96));
        assert_eq!(150, to_byte_idx(text, 120));
        assert_eq!(180, to_byte_idx(text, 144));
        assert_eq!(210, to_byte_idx(text, 168));
        assert_eq!(239, to_byte_idx(text, 191));
    }

    #[test]
    fn to_byte_idx_05() {
        // Ascii range
        for i in 0..88 {
            assert_eq!(i, to_byte_idx(TEXT_LINES, i));
        }

        // Hiragana characters
        for i in 88..100 {
            assert_eq!(88 + ((i - 88) * 3), to_byte_idx(TEXT_LINES, i));
        }

        // Past the end
        for i in 100..110 {
            assert_eq!(124, to_byte_idx(TEXT_LINES, i));
        }
    }
}
