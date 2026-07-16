pub fn joaat_hash(input: &str, literal: bool) -> u32 {
    let mut hash: u32 = 0;

    for byte in input.bytes() {
        let b = if literal { byte } else { byte.to_ascii_lowercase() };
        hash = hash.wrapping_add(b as u32);
        hash = hash.wrapping_add(hash << 10);
        hash ^= hash >> 6;
    }

    hash = hash.wrapping_add(hash << 3);
    hash ^= hash >> 11;
    hash = hash.wrapping_add(hash << 15);
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_hash_is_ascii_case_insensitive() {
        assert_eq!(joaat_hash("Adder", false), joaat_hash("adder", false));
    }

    #[test]
    fn literal_hash_is_case_sensitive() {
        assert_ne!(joaat_hash("Adder", true), joaat_hash("adder", true));
    }
}
