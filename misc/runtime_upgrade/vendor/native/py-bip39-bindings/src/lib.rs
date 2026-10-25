// Python BIP39 Bindings
//
// Copyright 2018-2020 Stichting Polkascan (Polkascan Foundation).
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Python bindings for the tiny-bip39 crate
//!
//! py-bip39-bindings provides bindings to the Rust create
//! [tiny-bip39](https://crates.io/crates/tiny-bip39), allowing mnemonic generation, validation and
//! conversion to seed and mini-secret.

use pyo3::exceptions;
use pyo3::prelude::*;

use ::bip39::{Mnemonic, Language, MnemonicType, Seed};
use hmac::Hmac;
use pbkdf2::pbkdf2;
use sha2::Sha512;

/// Create a mini-secret from a BIP39 phrase
///
/// # Arguments
///
/// * `phrase` - Mnemonic phrase
/// * `password` - Use empty string for no password
/// * `language_code` - The language to use, valid values are: 'en', 'zh-hans', 'zh-hant', 'fr', 'it', 'ja', 'ko', 'es'. Defaults to 'en'
///
/// # Returns
///
/// Returns the 32-bytes mini-secret via entropy
#[pyfunction]
#[pyo3(signature = (phrase, password, language_code="en"))]
pub fn bip39_to_mini_secret(phrase: &str, password: &str, language_code: Option<&str>) -> PyResult<Vec<u8>> {
	let salt = format!("mnemonic{}", password);

	let language = match Language::from_language_code(language_code.unwrap_or("en")) {
		Some(language) => language,
		None => return Err(exceptions::PyValueError::new_err("Invalid language_code"))
	};

	let mnemonic = match Mnemonic::from_phrase(phrase, language) {
		Ok(some_mnemomic) => some_mnemomic,
		Err(err) => return Err(exceptions::PyValueError::new_err(format!("Invalid mnemonic: {}", err.to_string())))
	};
	let mut result = [0u8; 64];

	let _ = pbkdf2::<Hmac<Sha512>>(mnemonic.entropy(), salt.as_bytes(), 2048, &mut result);

	Ok(result[..32].to_vec())
}

/// Generates a new mnemonic
///
/// # Arguments
///
/// * `words` - The amount of words to generate, valid values are 12, 15, 18, 21 and 24

///
/// # Returns
///
/// A string containing the mnemonic words.
#[pyfunction]
#[pyo3(signature = (words, language_code="en"))]
pub fn bip39_generate(words: u32, language_code: Option<&str>) -> PyResult<String> {

	let language = match Language::from_language_code(language_code.unwrap_or("en")) {
		Some(language) => language,
		None => return Err(exceptions::PyValueError::new_err("Invalid language_code"))
	};

	let word_count_type = match MnemonicType::for_word_count(words as usize) {
		Ok(some_work_count) => some_work_count,
		Err(err) => return Err(exceptions::PyValueError::new_err(err.to_string()))
	};

	let phrase = Mnemonic::new(word_count_type, language).into_phrase();

	assert_eq!(phrase.split(" ").count(), words as usize);

	Ok(phrase.to_owned())
}

/// Creates a seed from a BIP39 phrase
///
/// # Arguments
///
/// * `phrase` - Mnemonic phrase
/// * `password` - Use empty string for no password
/// * `language_code` - The language to use, valid values are: 'en', 'zh-hans', 'zh-hant', 'fr', 'it', 'ja', 'ko', 'es'. Defaults to 'en'
///
/// # Returns
///
/// Returns a 32-bytes seed
///
#[pyfunction]
#[pyo3(signature = (phrase, password, language_code="en"))]
pub fn bip39_to_seed(phrase: &str, password: &str, language_code: Option<&str>) -> PyResult<Vec<u8>> {

	let language = match Language::from_language_code(language_code.unwrap_or("en")) {
		Some(language) => language,
		None => return Err(exceptions::PyValueError::new_err("Invalid language_code"))
	};

	let mnemonic = match Mnemonic::from_phrase(phrase, language) {
		Ok(some_mnemomic) => some_mnemomic,
		Err(err) => return Err(exceptions::PyValueError::new_err(format!("Invalid mnemonic: {}", err.to_string())))
	};

	Ok(Seed::new(&mnemonic, password)
		.as_bytes()[..32]
		.to_vec())
}


/// Validates a BIP39 phrase
///
/// # Arguments
///
/// * `phrase` - Mnemonic phrase
/// * `language_code` - The language to use, valid values are: 'en', 'zh-hans', 'zh-hant', 'fr', 'it', 'ja', 'ko', 'es'. Defaults to 'en'
///
/// # Returns
///
/// Returns boolean with validation result
#[pyfunction]
#[pyo3(signature = (phrase, language_code="en"))]
pub fn bip39_validate(phrase: &str, language_code: Option<&str>) -> PyResult<bool> {
	let language = match Language::from_language_code(language_code.unwrap_or("en")) {
		Some(language) => language,
		None => return Err(exceptions::PyValueError::new_err("Invalid language_code"))
	};

	match Mnemonic::validate(phrase, language) {
		Err(_) => Ok(false),
		_ => Ok(true)
	}
}

#[pymodule]
fn bip39(m: &Bound<'_, PyModule>) -> PyResult<()> {
	m.add_function(wrap_pyfunction!(bip39_to_mini_secret, m)?)?;
	m.add_function(wrap_pyfunction!(bip39_generate, m)?)?;
	m.add_function(wrap_pyfunction!(bip39_to_seed, m)?)?;
	m.add_function(wrap_pyfunction!(bip39_validate, m)?)?;
    Ok(())
}
