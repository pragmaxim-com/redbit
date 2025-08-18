use crate::ChainError;

pub fn ensure_hashes_equal(expected_prev_hash: String, actual_prev_hash: String, height: u32) -> Result<(), ChainError> {
    if expected_prev_hash != actual_prev_hash {
        return Err(ChainError::new(
            format!(
                "Chain corrupted at height {}: expected prev_hash {} but got {}. Missing link to header with hash {}.",
                height, expected_prev_hash, actual_prev_hash, expected_prev_hash
            )
        ));
    }
    Ok(())
}
