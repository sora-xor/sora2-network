#[derive(Debug)]
pub struct Proof<T> {
    pub order: u64,
    pub items: Vec<T>,
}

impl<T: Clone> Proof<T> {
    pub fn root(&self, hash: impl Fn(T, T) -> T, node_hash: T) -> T {
        let mut current_hash = node_hash;

        for (i, item) in self.items.iter().cloned().enumerate() {
            let is_sibling_left = (self.order >> i) & 1 == 1;

            if is_sibling_left {
                current_hash = hash(item, current_hash);
            } else {
                current_hash = hash(current_hash, item);
            }
        }
        current_hash
    }
}

pub fn leaf_index_to_pos(index: u64) -> u64 {
    leaf_index_to_mmr_size(index) - (index + 1).trailing_zeros() as u64 - 1
}

pub fn leaf_count_to_mmr_size(leaves_count: u64) -> u64 {
    let peak_count = leaves_count.count_ones() as u64;
    2 * leaves_count - peak_count
}

fn leaf_index_to_mmr_size(index: u64) -> u64 {
    leaf_count_to_mmr_size(index + 1)
}

pub fn pos_height_in_tree(mut pos: u64) -> u32 {
    pos += 1;
    fn all_ones(num: u64) -> bool {
        num != 0 && num.count_zeros() == num.leading_zeros()
    }
    fn jump_left(pos: u64) -> u64 {
        let bit_length = 64 - pos.leading_zeros();
        let most_significant_bits = 1 << (bit_length - 1);
        pos - (most_significant_bits - 1)
    }

    while !all_ones(pos) {
        pos = jump_left(pos)
    }

    64 - pos.leading_zeros() - 1
}

pub fn parent_offset(height: u32) -> u64 {
    2 << height
}

pub fn sibling_offset(height: u32) -> u64 {
    (2 << height) - 1
}

pub fn get_peaks(mmr_size: u64) -> Vec<u64> {
    let mut pos_s = Vec::new();
    let (mut height, mut pos) = left_peak_height_pos(mmr_size);
    pos_s.push(pos);
    while height > 0 {
        let peak = match get_right_peak(height, pos, mmr_size) {
            Some(peak) => peak,
            None => break,
        };
        height = peak.0;
        pos = peak.1;
        pos_s.push(pos);
    }
    pos_s
}

fn get_right_peak(mut height: u32, mut pos: u64, mmr_size: u64) -> Option<(u32, u64)> {
    // move to right sibling pos
    pos += sibling_offset(height);
    // loop until we find a pos in mmr
    while pos > mmr_size - 1 {
        if height == 0 {
            return None;
        }
        // move to left child
        pos -= parent_offset(height - 1);
        height -= 1;
    }
    Some((height, pos))
}

fn get_peak_pos_by_height(height: u32) -> u64 {
    (1 << (height + 1)) - 2
}

fn left_peak_height_pos(mmr_size: u64) -> (u32, u64) {
    let mut height = 1;
    let mut prev_pos = 0;
    let mut pos = get_peak_pos_by_height(height);
    while pos < mmr_size {
        height += 1;
        prev_pos = pos;
        pos = get_peak_pos_by_height(height);
    }
    (height - 1, prev_pos)
}

fn calculate_merkle_proof_order<T>(leave: u64, proof: &Vec<T>) -> u64 {
    let mut order = 0u64;
    let mut tree_pos = leave;

    for i in 0..proof.len() as u32 {
        if pos_height_in_tree(tree_pos + 1) > i {
            order |= 1 << i;
            tree_pos = tree_pos + parent_offset(i) - sibling_offset(i);
        } else {
            tree_pos += sibling_offset(i) + 1;
        };
    }
    order
}

#[allow(dead_code)]
pub fn convert_to_simplified_mmr_proof<T: Clone>(
    leaf_index: u64,
    leaf_count: u64,
    proof_items: &[T],
) -> Proof<T> {
    let leaf_pos = leaf_index_to_pos(leaf_index);

    let mut ready_made_peak_hashes = vec![];
    let mut optional_right_bagged_peak = None;
    let mut merkle_proof = vec![];

    let mut proof_item_position = 0;
    let mut merkle_root_peak_position = 0;

    let mmr_size = leaf_count_to_mmr_size(leaf_count);
    let peaks = get_peaks(mmr_size);

    for i in 0..peaks.len() {
        if (i == 0 || leaf_pos > peaks[i - 1]) && leaf_pos <= peaks[i] {
            merkle_root_peak_position = i;
            if i == peaks.len() - 1 {
                for i in proof_item_position..proof_items.len() {
                    merkle_proof.push(proof_items[i].clone());
                }
            } else {
                for i in proof_item_position..proof_items.len() - 1 {
                    merkle_proof.push(proof_items[i].clone());
                }
                optional_right_bagged_peak = Some(proof_items[proof_items.len() - 1].clone());
                break;
            }
        } else {
            ready_made_peak_hashes.push(proof_items[proof_item_position].clone());
            proof_item_position += 1
        }
    }

    let localized_merkle_root_position = if merkle_root_peak_position == 0 {
        leaf_pos
    } else {
        leaf_pos - peaks[merkle_root_peak_position - 1] - 1
    };

    let mut proof_order =
        calculate_merkle_proof_order(localized_merkle_root_position, &merkle_proof);

    // Adding peaks into merkle proof itself
    if let Some(optional_right_bagged_peak) = optional_right_bagged_peak {
        proof_order |= 1 << merkle_proof.len();
        merkle_proof.push(optional_right_bagged_peak);
    }
    for peak in ready_made_peak_hashes.into_iter().rev() {
        merkle_proof.push(peak);
    }

    Proof {
        order: proof_order,
        items: merkle_proof,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bridge_types::H256;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Serialize, Deserialize)]
    struct SimplifiedProofTestData {
        #[serde(rename = "LeafIndex")]
        leaf_index: Option<u64>,
        #[serde(rename = "LeafCount")]
        leaf_count: Option<u64>,
        #[serde(rename = "MMRProof")]
        mmr_proof: Option<Vec<Vec<u8>>>,
        #[serde(rename = "SimplifiedMerkleProofItems")]
        proof_items: Option<Vec<Vec<u8>>>,
        #[serde(rename = "SimplifiedMerkleProofOrder")]
        proof_order: Option<u64>,
    }

    #[test]
    fn test_simplified_mmr_proof() {
        let bytes = include_bytes!("./simplified_proof_fixture.json");
        let test_data = serde_json::from_slice::<Vec<SimplifiedProofTestData>>(bytes).unwrap();
        let mut passed = 0;
        for item in test_data {
            if item.leaf_count.is_none()
                || item.leaf_index.is_none()
                || item.mmr_proof.is_none()
                || item.proof_items.is_none()
                || item.proof_order.is_none()
            {
                continue;
            }
            let simplified_proof = convert_to_simplified_mmr_proof(
                item.leaf_index.unwrap(),
                item.leaf_count.unwrap(),
                &item
                    .mmr_proof
                    .unwrap()
                    .into_iter()
                    .map(|x| H256::from_slice(&x))
                    .collect::<Vec<_>>(),
            );
            assert_eq!(
                simplified_proof.order,
                item.proof_order.unwrap(),
                "passed {}, leafs {:?}, index {:?}",
                passed,
                item.leaf_count,
                item.leaf_index
            );
            assert_eq!(
                simplified_proof.items,
                item.proof_items
                    .unwrap()
                    .into_iter()
                    .map(|x| H256::from_slice(&x))
                    .collect::<Vec<_>>(),
                "passed {}, leafs {:?}, index {:?}",
                passed,
                item.leaf_count,
                item.leaf_index
            );
            passed += 1;
        }
        assert_ne!(passed, 0);
    }
}
