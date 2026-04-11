const CHUNK_SIZE: usize = 8 * 1024; // 8KB

/// テキストを行単位で分割し、各チャンクが CHUNK_SIZE バイト前後になるようにする。
/// 行の途中で切らないため、マルチバイト文字でも安全。
pub fn chunk(text: &str) -> Vec<&str> {
    if text.len() <= CHUNK_SIZE {
        return vec![text];
    }

    let mut chunks = Vec::new();
    let mut chunk_start = 0;
    let mut current_size = 0;
    let mut last_newline = 0;

    for (i, c) in text.char_indices() {
        if c == '\n' {
            last_newline = i + 1; // '\n' の次のバイト位置
        }
        current_size += c.len_utf8();
        if current_size >= CHUNK_SIZE && last_newline > chunk_start {
            chunks.push(&text[chunk_start..last_newline]);
            chunk_start = last_newline;
            current_size = i + c.len_utf8() - last_newline;
        }
    }

    if chunk_start < text.len() {
        chunks.push(&text[chunk_start..]);
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_text_is_single_chunk() {
        let text = "hello world";
        let chunks = chunk(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "hello world");
    }

    #[test]
    fn large_text_is_split() {
        let line = "abcdefghij\n"; // 11 bytes
        let text = line.repeat(1000); // 11KB
        let chunks = chunk(&text);
        assert!(chunks.len() > 1);
    }

    #[test]
    fn chunks_reconstruct_original() {
        let line = "abcdefghij\n";
        let text = line.repeat(1000);
        let chunks = chunk(&text);
        let reconstructed: String = chunks.concat();
        assert_eq!(reconstructed, text);
    }

    #[test]
    fn handles_multibyte_characters() {
        // 日本語テキスト（1文字3バイト）で境界を跨がないことを確認
        let line = "あいうえおかきくけこ\n"; // 31 bytes per line
        let text = line.repeat(500); // ~15KB
        let chunks = chunk(&text);
        assert!(chunks.len() > 1);
        let reconstructed: String = chunks.concat();
        assert_eq!(reconstructed, text);
    }
}
