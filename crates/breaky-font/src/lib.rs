pub const ASCENT: f32 = 5.0;
pub const DESCENT: f32 = 2.0;
pub const EM: f32 = 8.0;

pub struct Glyph {
    pub width: u8,
    pub rows: [u8; 7],
}

const TABLE: [Glyph; 95] = [
    Glyph {
        width: 2,
        rows: [0, 0, 0, 0, 0, 0, 0],
    },
    Glyph {
        width: 1,
        rows: [0b1, 0b1, 0b1, 0, 0b1, 0, 0],
    },
    Glyph {
        width: 3,
        rows: [0b101, 0b101, 0, 0, 0, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b00100, 0b11110, 0b01001, 0b01110, 0b10100, 0b01111, 0b00100],
    },
    Glyph {
        width: 5,
        rows: [0b11001, 0b11010, 0b00100, 0b01011, 0b10011, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b00110, 0b01001, 0b00101, 0b00010, 0b10101, 0b01001, 0b10110],
    },
    Glyph {
        width: 1,
        rows: [0b1, 0b1, 0, 0, 0, 0, 0],
    },
    Glyph {
        width: 2,
        rows: [0b10, 0b01, 0b01, 0b01, 0b01, 0b01, 0b10],
    },
    Glyph {
        width: 2,
        rows: [0b01, 0b10, 0b10, 0b10, 0b10, 0b10, 0b01],
    },
    Glyph {
        width: 5,
        rows: [0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
    },
    Glyph {
        width: 1,
        rows: [0, 0, 0, 0, 0b1, 0b1, 0],
    },
    Glyph {
        width: 3,
        rows: [0, 0, 0, 0b111, 0, 0, 0],
    },
    Glyph {
        width: 1,
        rows: [0, 0, 0, 0, 0b1, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b00001, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000],
    },
    Glyph {
        width: 4,
        rows: [0b0110, 0b1001, 0b1001, 0b1001, 0b0110, 0, 0],
    },
    Glyph {
        width: 3,
        rows: [0b0010, 0b0110, 0b0010, 0b0010, 0b0111, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0b0110, 0b1001, 0b0100, 0b0010, 0b1111, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0b1110, 0b1000, 0b0110, 0b1000, 0b1110, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0b1001, 0b1001, 0b1111, 0b1000, 0b1000, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0b1111, 0b0001, 0b1110, 0b1000, 0b1110, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0b0110, 0b0001, 0b1111, 0b1001, 0b0110, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0b1111, 0b1000, 0b0100, 0b0100, 0b0100, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0b0110, 0b1001, 0b0110, 0b1001, 0b0110, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0b0110, 0b1001, 0b1111, 0b1000, 0b0110, 0, 0],
    },
    Glyph {
        width: 1,
        rows: [0, 0b1, 0, 0, 0b1, 0, 0],
    },
    Glyph {
        width: 1,
        rows: [0, 0b1, 0, 0, 0b1, 0b1, 0],
    },
    Glyph {
        width: 3,
        rows: [0b100, 0b010, 0b001, 0b010, 0b100, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0, 0b1111, 0, 0b1111, 0, 0, 0],
    },
    Glyph {
        width: 3,
        rows: [0b001, 0b010, 0b100, 0b010, 0b001, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0b0110, 0b1001, 0b0100, 0b0100, 0, 0b0100, 0],
    },
    Glyph {
        width: 5,
        rows: [0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01110],
    },
    Glyph {
        width: 5,
        rows: [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b11110, 0b10001, 0b11110, 0b10001, 0b11110, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b01110, 0b10001, 0b00001, 0b10001, 0b01110, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b11110, 0b10001, 0b10001, 0b10001, 0b11110, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b11111, 0b00001, 0b11110, 0b00001, 0b11111, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b11111, 0b00001, 0b11110, 0b00001, 0b00001, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b01110, 0b00001, 0b00001, 0b10011, 0b01110, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0, 0],
    },
    Glyph {
        width: 3,
        rows: [0b111, 0b010, 0b010, 0b010, 0b111, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0b1110, 0b1000, 0b1000, 0b1001, 0b0110, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b10001, 0b01001, 0b00101, 0b01001, 0b10001, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b00001, 0b00001, 0b00001, 0b00001, 0b11111, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b10001, 0b11011, 0b10101, 0b10001, 0b10001, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b01110, 0b10001, 0b10001, 0b10001, 0b01110, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b11110, 0b10001, 0b11110, 0b00001, 0b00001, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b01110, 0b10001, 0b10101, 0b10010, 0b01101, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b11110, 0b10001, 0b11110, 0b10011, 0b10001, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b01111, 0b00001, 0b01110, 0b10000, 0b11110, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b10001, 0b10001, 0b10001, 0b10001, 0b01110, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b10001, 0b10001, 0b10001, 0b01010, 0b00100, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b10001, 0b10001, 0b10101, 0b11011, 0b10001, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0b11111, 0b00010, 0b00100, 0b01000, 0b11111, 0, 0],
    },
    Glyph {
        width: 2,
        rows: [0b11, 0b10, 0b10, 0b10, 0b10, 0b10, 0b11],
    },
    Glyph {
        width: 5,
        rows: [0b10000, 0b10000, 0b01000, 0b00100, 0b00010, 0b00001, 0b00001],
    },
    Glyph {
        width: 2,
        rows: [0b11, 0b01, 0b01, 0b01, 0b01, 0b01, 0b11],
    },
    Glyph {
        width: 5,
        rows: [0b00100, 0b01010, 0b10001, 0, 0, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0, 0, 0, 0, 0, 0b11111, 0],
    },
    Glyph {
        width: 2,
        rows: [0b10, 0b01, 0, 0, 0, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0, 0b0110, 0b1001, 0b1001, 0b0111, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0b0001, 0b0001, 0b0111, 0b1001, 0b0111, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0, 0b0110, 0b0001, 0b0001, 0b0110, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0b1000, 0b1000, 0b1110, 0b1001, 0b1110, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0, 0b0110, 0b1111, 0b0001, 0b0110, 0, 0],
    },
    Glyph {
        width: 3,
        rows: [0b0110, 0b0111, 0b0010, 0b0010, 0b0010, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0, 0b0110, 0b1001, 0b1001, 0b0111, 0b0001, 0b0110],
    },
    Glyph {
        width: 4,
        rows: [0b0001, 0b0001, 0b0111, 0b1001, 0b1001, 0, 0],
    },
    Glyph {
        width: 1,
        rows: [0b1, 0, 0b1, 0b1, 0b1, 0, 0],
    },
    Glyph {
        width: 2,
        rows: [0b10, 0b10, 0b10, 0b10, 0b10, 0b10, 0b01],
    },
    Glyph {
        width: 3,
        rows: [0b001, 0b001, 0b101, 0b011, 0b101, 0, 0],
    },
    Glyph {
        width: 1,
        rows: [0b1, 0b1, 0b1, 0b1, 0b1, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0, 0b11011, 0b10101, 0b10101, 0b10101, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0, 0b0111, 0b1001, 0b1001, 0b1001, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0, 0b0110, 0b1001, 0b1001, 0b0110, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0, 0b0111, 0b1001, 0b1001, 0b0111, 0b0001, 0b0001],
    },
    Glyph {
        width: 4,
        rows: [0, 0b1110, 0b1001, 0b1001, 0b1110, 0b1000, 0b1000],
    },
    Glyph {
        width: 3,
        rows: [0, 0b0111, 0b0001, 0b0001, 0b0001, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0, 0b1110, 0b0001, 0b1110, 0b0111, 0, 0],
    },
    Glyph {
        width: 3,
        rows: [0b0010, 0b0111, 0b0010, 0b0010, 0b0110, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0, 0b1001, 0b1001, 0b1001, 0b0111, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0, 0b10001, 0b10001, 0b01010, 0b00100, 0, 0],
    },
    Glyph {
        width: 5,
        rows: [0, 0b10001, 0b10001, 0b10101, 0b01010, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0, 0b1001, 0b0110, 0b0110, 0b1001, 0, 0],
    },
    Glyph {
        width: 4,
        rows: [0, 0b1001, 0b1001, 0b1001, 0b1000, 0b1000, 0b0110],
    },
    Glyph {
        width: 4,
        rows: [0, 0b1111, 0b0010, 0b0100, 0b1111, 0, 0],
    },
    Glyph {
        width: 2,
        rows: [0b10, 0b10, 0b10, 0b01, 0b10, 0b10, 0b10],
    },
    Glyph {
        width: 1,
        rows: [0b1, 0b1, 0b1, 0b1, 0b1, 0b1, 0b1],
    },
    Glyph {
        width: 2,
        rows: [0b01, 0b01, 0b01, 0b10, 0b01, 0b01, 0b01],
    },
    Glyph {
        width: 5,
        rows: [0, 0, 0, 0b01001, 0b10010, 0, 0],
    },
];

const DISC: Glyph = Glyph {
    width: 3,
    rows: [0b010, 0b111, 0b111, 0b010, 0, 0, 0],
};

const RING: Glyph = Glyph {
    width: 3,
    rows: [0b010, 0b101, 0b101, 0b010, 0, 0, 0],
};

const BLOCKY: Glyph = Glyph {
    width: 3,
    rows: [0b111, 0b111, 0b111, 0b111, 0, 0, 0],
};

pub fn glyph(c: char) -> &'static Glyph {
    match c {
        '\u{2022}' => &DISC,
        '\u{25e6}' => &RING,
        '\u{25aa}' => &BLOCKY,
        _ => table_glyph(c),
    }
}

fn table_glyph(c: char) -> &'static Glyph {
    let u = c as u32;
    if (32..127).contains(&u) {
        &TABLE[u as usize - 32]
    } else if matches!(c, '\t' | '\n' | '\r' | '\u{a0}') {
        &TABLE[' ' as usize - 32]
    } else {
        &TABLE['?' as usize - 32]
    }
}

pub fn scale(size: f32) -> f32 {
    size / EM
}

pub fn advance(c: char, size: f32) -> f32 {
    match c {
        '\n' | '\r' => 0.0,
        '\t' => 4.0 * advance(' ', size),
        _ => {
            let g = glyph(c);
            (g.width as f32 + 1.0) * scale(size)
        }
    }
}

pub fn width(text: &str, size: f32) -> f32 {
    text.chars().map(|c| advance(c, size)).sum()
}

pub fn asc(size: f32) -> f32 {
    ASCENT * scale(size)
}

pub fn desc(size: f32) -> f32 {
    DESCENT * scale(size)
}

pub fn x_height(size: f32) -> f32 {
    4.0 * scale(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proportional_widths() {
        let s = 16.0;
        assert!(width("ii", s) < width("ww", s));
        assert!(width(" ", s) > 0.0);
    }

    #[test]
    fn width_scales_with_size() {
        assert!((width("abc", 16.0) - width("abc", 8.0) * 2.0).abs() < 1e-4);
    }

    #[test]
    fn unknown_chars_fall_back() {
        let q = glyph('?');
        assert!(std::ptr::eq(glyph('\u{436}'), q));
        assert!(std::ptr::eq(glyph('\u{1f600}'), q));
    }

    #[test]
    fn control_whitespace_maps_to_space() {
        assert!(std::ptr::eq(glyph('\t'), glyph(' ')));
        assert!(std::ptr::eq(glyph('\n'), glyph(' ')));
        assert!(std::ptr::eq(glyph('\u{a0}'), glyph(' ')));
        assert!((advance('\t', 16.0) - 4.0 * advance(' ', 16.0)).abs() < 1e-4);
        assert_eq!(advance('\n', 16.0), 0.0);
        assert_eq!(advance('\r', 16.0), 0.0);
        assert!((advance('\u{a0}', 16.0) - advance(' ', 16.0)).abs() < 1e-4);
    }

    #[test]
    fn vertical_metrics() {
        assert!((asc(16.0) - 10.0).abs() < 1e-4);
        assert!((desc(16.0) - 4.0).abs() < 1e-4);
        assert!((asc(32.0) - 20.0).abs() < 1e-4);
    }

    #[test]
    fn glyph_shape() {
        let a = glyph('A');
        assert_eq!(a.width, 5);
        assert_eq!(a.rows[0], 0b01110);
        assert_eq!(a.rows[3], 0b11111);
        let dot = glyph('.');
        assert_eq!(dot.rows[4], 0b1);
        assert_eq!(dot.rows[0], 0);
    }

    #[test]
    fn space_is_blank() {
        let g = glyph(' ');
        assert!(g.rows.iter().all(|r| *r == 0));
        assert!((advance(' ', 16.0) - 6.0).abs() < 1e-4);
    }

    #[test]
    fn empty_text() {
        assert_eq!(width("", 16.0), 0.0);
    }
}
