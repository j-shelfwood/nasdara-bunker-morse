// International Morse digits. Assumed (not yet confirmed against the engine): the native
// BroadcastTextAsMorse encodes digits with the standard five-element codes. Bunker snapping
// (bunkers.rs) is the cross-check: a wrong table makes decoded grids stop matching real bunkers.

/// Symbols and digit, in table order (the grid decoder breaks score ties by this order).
pub const DIGITS: [(&str, char); 10] = [
    ("-----", '0'),
    (".----", '1'),
    ("..---", '2'),
    ("...--", '3'),
    ("....-", '4'),
    (".....", '5'),
    ("-....", '6'),
    ("--...", '7'),
    ("---..", '8'),
    ("----.", '9'),
];

/// Morse symbols of a digit; the empty string for anything that is not a decimal digit.
pub fn pattern_of(digit: char) -> &'static str {
    DIGITS
        .iter()
        .find(|(_, d)| *d == digit)
        .map_or("", |(symbols, _)| symbols)
}

/// The digit for a symbol string, `'?'` when it is not one of the ten codes.
pub fn digit_for(symbols: &str) -> char {
    DIGITS
        .iter()
        .find(|(s, _)| *s == symbols)
        .map_or('?', |(_, d)| *d)
}
