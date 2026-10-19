//! Which pen tablets the Linux kernel knows (step 58), read from `/proc/bus/input/devices` (world-readable, no
//! root, no extra crate). Used before the window opens to decide whether a Wayland session should open the window
//! through Xwayland, where the XInput2 reader gets pressure.
//!
//! A pen is an input device whose key bitmap has `BTN_TOOL_PEN` and whose absolute-axis bitmap has `ABS_PRESSURE`
//! (the evdev codes every tablet driver reports: wacom, hid-generic for Huion/XP-Pen/Gaomon, uinput devices of
//! OpenTabletDriver). Touchscreens without a pen and the tablet's pad (buttons, ring) don't qualify.

/// `BTN_TOOL_PEN` (linux/input-event-codes.h).
const BTN_TOOL_PEN: usize = 0x140;
/// `ABS_PRESSURE`.
const ABS_PRESSURE: usize = 0x18;
/// Pens kept (a hostile or broken file could list thousands of devices).
const MAX_PENS: usize = 16;

/// The names of the pen devices in `/proc/bus/input/devices` (empty when it can't be read, e.g. not Linux).
pub fn connected_pens() -> Vec<String> {
    std::fs::read_to_string("/proc/bus/input/devices").map(|text| pens(&text, usize::BITS as usize)).unwrap_or_default()
}

/// The names of the pen devices in `text` (the file's format: blocks of `X: …` lines separated by blank lines;
/// bitmaps are hexadecimal words of `word_bits` bits, the most significant word first).
pub fn pens(text: &str, word_bits: usize) -> Vec<String> {
    text.split("\n\n").filter_map(|block| pen_name(block, word_bits)).take(MAX_PENS).collect()
}

/// The device name of `block` if it is a pen.
fn pen_name(block: &str, word_bits: usize) -> Option<String> {
    let mut name = None;
    let (mut pen, mut pressure) = (false, false);
    for line in block.lines() {
        if let Some(n) = line.strip_prefix("N: Name=") {
            name = Some(n.trim().trim_matches('"').to_string());
        } else if let Some(bits) = line.strip_prefix("B: KEY=") {
            pen = has_bit(bits, BTN_TOOL_PEN, word_bits);
        } else if let Some(bits) = line.strip_prefix("B: ABS=") {
            pressure = has_bit(bits, ABS_PRESSURE, word_bits);
        }
    }
    if pen && pressure { Some(name.unwrap_or_else(|| "Unnamed pen".to_string())) } else { None }
}

/// Whether bit `bit` is set in the bitmap `words` (hex words, most significant first).
fn has_bit(words: &str, bit: usize, word_bits: usize) -> bool {
    if word_bits == 0 {
        return false;
    }
    let (index, shift) = (bit / word_bits, bit % word_bits);
    let Some(word) = words.split_whitespace().rev().nth(index) else { return false };
    // a word wider than u128 is malformed: no pen
    u128::from_str_radix(word, 16).is_ok_and(|w| shift < 128 && (w >> shift) & 1 == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tablet's pen, its pad, a mouse and a keyboard as a 64-bit kernel lists them (bitmaps from the evdev
    /// codes, names made up).
    const DEVICES: &str = "I: Bus=0003 Vendor=056a Product=0001 Version=0110
N: Name=\"Tablet Maker Pen Tablet Pen\"
H: Handlers=mouse2 event10
B: PROP=1
B: EV=1b
B: KEY=1c03 0 0 0 0 0
B: ABS=d000003
B: MSC=1

I: Bus=0003 Vendor=056a Product=0001 Version=0110
N: Name=\"Tablet Maker Pen Tablet Pad\"
B: EV=1b
B: KEY=3f 0 0 0 0 0 0
B: ABS=3000003

I: Bus=0003 Vendor=046d Product=c52b Version=0111
N: Name=\"Some USB Mouse\"
B: EV=17
B: KEY=ffff0000 0 0 0 0
B: REL=1943

I: Bus=0003 Vendor=046d Product=c52b Version=0111
N: Name=\"Touchscreen without pen\"
B: EV=b
B: KEY=400 0 0 0 0 0
B: ABS=3273800000000003
";

    #[test]
    fn only_devices_with_a_pen_tool_and_pressure_are_pens() {
        assert_eq!(pens(DEVICES, 64), vec!["Tablet Maker Pen Tablet Pen".to_string()]);
    }

    #[test]
    fn bitmaps_are_read_from_the_least_significant_word() {
        // 0x140 = word 5 bit 0 with 64-bit words, word 10 bit 0 with 32-bit words
        assert!(has_bit("1 0 0 0 0 0", BTN_TOOL_PEN, 64));
        assert!(!has_bit("1 0 0 0 0", BTN_TOOL_PEN, 64));
        assert!(has_bit("1 0 0 0 0 0 0 0 0 0 0", BTN_TOOL_PEN, 32));
        assert!(has_bit("1000000", ABS_PRESSURE, 64));
        assert!(!has_bit("800000", ABS_PRESSURE, 64));
    }

    #[test]
    fn damaged_input_is_no_pen() {
        assert!(pens("", 64).is_empty());
        assert!(pens("N: Name=\"x\"\nB: KEY=zz 0 0 0 0 0\nB: ABS=1000000", 64).is_empty());
        assert!(!has_bit("1", 0, 0));
        assert!(!has_bit("fffffffffffffffffffffffffffffffffffffffff", 3, 64));
        // a pen without a name still counts
        assert_eq!(pens("B: KEY=1 0 0 0 0 0\nB: ABS=1000000", 64), vec!["Unnamed pen".to_string()]);
        let many = "B: KEY=1 0 0 0 0 0\nB: ABS=1000000\n\n".repeat(100);
        assert_eq!(pens(&many, 64).len(), MAX_PENS);
    }
}
