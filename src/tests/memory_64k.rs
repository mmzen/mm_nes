#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_memory_with_zero_size() {
        let mut memory = Memory64k {
            memory: [0; 0], // Set memory size to 0
        };

        let result = memory.initialize();

        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), MemoryError::InvalidSize);
    }
}
