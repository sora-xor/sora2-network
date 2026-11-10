// Smoldot
// Copyright (C) 2019-2022  Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

#![cfg(test)]

#[test]
fn basic_works() {
    // Note that this example file doesn't have the zstd header.
    super::zstd_decode(
        &include_bytes!("./example-runtime.wasm.zstd")[..],
        10 * 1024 * 1024,
    )
    .unwrap();
}

#[test]
fn limit_reached() {
    // Note that this example file doesn't have the zstd header.
    assert!(matches!(
        super::zstd_decode(
            &include_bytes!("./example-runtime.wasm.zstd")[..],
            16 * 1024
        ),
        Err(super::Error::TooLarge)
    ));
}

#[test]
fn invalid_data() {
    assert!(matches!(
        super::zstd_decode(&(0..2048).map(|_| 0xff).collect::<Vec<_>>(), 1024 * 1024),
        Err(super::Error::InvalidZstd)
    ));
}

#[test]
fn basic_works_with_header() {
    super::zstd_decode_if_necessary(
        &include_bytes!("./polkadot-runtime-v9160.wasm.zstd")[..],
        10 * 1024 * 1024,
    )
    .unwrap();
}

#[test]
fn excessive_window_is_rejected_before_allocation() {
    // A valid Zstandard frame header can advertise a multi-terabyte window even
    // when the frame contains only an empty last block. The decoder must reject
    // the declared window before allocating its history buffer.
    let frame = [0x28, 0xb5, 0x2f, 0xfd, 0x00, 0xff, 0x01, 0x00, 0x00];
    let mut runtime = super::ZSTD_PREFIX.to_vec();
    runtime.extend_from_slice(&frame);
    assert!(matches!(
        super::zstd_decode_if_necessary(&runtime, 1024),
        Err(super::Error::InvalidZstd)
    ));
}

#[test]
fn final_block_must_respect_output_limit() {
    // Single-segment frame with one final raw block containing ten bytes.
    // UptoBytes is a block-level stopping hint, not a hard output-size cap.
    let frame = [
        0x28, 0xb5, 0x2f, 0xfd, 0x20, 10, 0x51, 0x00, 0x00, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
    ];
    assert_eq!(
        super::zstd_decode(&frame, 10).unwrap(),
        (0..10).collect::<Vec<_>>()
    );
    assert!(matches!(
        super::zstd_decode(&frame, 4),
        Err(super::Error::TooLarge)
    ));
}
