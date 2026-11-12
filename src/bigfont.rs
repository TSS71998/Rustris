pub const HEIGHT: usize = 5;

pub fn glyph(ch: char) -> Option<[&'static str; HEIGHT]> {
    Some(match ch {
        'A' => [".#.", "#.#", "###", "#.#", "#.#"],
        'B' => ["##.", "#.#", "##.", "#.#", "##."],
        'D' => ["##.", "#.#", "#.#", "#.#", "##."],
        'E' => ["###", "#..", "##.", "#..", "###"],
        'G' => [".##", "#..", "#.#", "#.#", ".##"],
        'I' => ["###", ".#.", ".#.", ".#.", "###"],
        'L' => ["#..", "#..", "#..", "#..", "###"],
        'M' => ["#.#", "###", "###", "#.#", "#.#"],
        'N' => ["##.", "#.#", "#.#", "#.#", "#.#"],
        'O' => [".#.", "#.#", "#.#", "#.#", ".#."],
        'P' => ["##.", "#.#", "##.", "#..", "#.."],
        'R' => ["##.", "#.#", "##.", "#.#", "#.#"],
        'S' => [".##", "#..", ".#.", "..#", "##."],
        'T' => ["###", ".#.", ".#.", ".#.", ".#."],
        'U' => ["#.#", "#.#", "#.#", "#.#", "###"],
        'V' => ["#.#", "#.#", "#.#", "#.#", ".#."],
        '!' => [".#.", ".#.", ".#.", "...", ".#."],
        ' ' => ["..", "..", "..", "..", ".."],
        _ => return None,
    })
}

pub fn for_each_pixel(text: &str, sx: usize, mut f: impl FnMut(usize, usize)) {
    let mut x0 = 0;
    for ch in text.chars() {
        let Some(rows) = glyph(ch) else { continue };
        for (y, row) in rows.iter().enumerate() {
            for (i, b) in row.bytes().enumerate() {
                if b == b'#' {
                    for k in 0..sx {
                        f(x0 + i * sx + k, y);
                    }
                }
            }
        }
        x0 += rows[0].len() * sx + 1;
    }
}

pub fn width(text: &str, sx: usize) -> usize {
    let w: usize = text
        .chars()
        .filter_map(glyph)
        .map(|rows| rows[0].len() * sx + 1)
        .sum();
    w.saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_have_equal_width() {
        for ch in "ABDEGILMNOPRSTUV! ".chars() {
            let rows = glyph(ch).unwrap();
            assert!(rows.iter().all(|r| r.len() == rows[0].len()), "{ch}");
        }
    }

    #[test]
    fn width_matches_pixels() {
        let mut max_x = 0;
        for_each_pixel("TETRIS", 2, |x, _| max_x = max_x.max(x));
        assert_eq!(width("TETRIS", 2), max_x + 1);
    }
}