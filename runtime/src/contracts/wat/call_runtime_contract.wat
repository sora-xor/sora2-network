;;This file is part of the SORA network and Polkaswap app.
;;
;;Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
;;SPDX-License-Identifier: BSD-4-Clause
;;
;;Redistribution and use in source and binary forms, with or without modification,
;;are permitted provided that the following conditions are met:
;;
;;Redistributions of source code must retain the above copyright notice, this list
;;of conditions and the following disclaimer.
;;Redistributions in binary form must reproduce the above copyright notice, this
;;list of conditions and the following disclaimer in the documentation and/or other
;;materials provided with the distribution.
;;
;;All advertising materials mentioning features or use of this software must display
;;the following acknowledgement: This product includes software developed by Polka Biome
;;Ltd., SORA, and Polkaswap.
;;
;;Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
;;to endorse or promote products derived from this software without specific prior written permission.
;;
;;THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
;;INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
;;A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
;;DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
;;BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
;;OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
;;STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
;;USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

;; This passes its input to `seal_call_runtime` and returns the return value to its caller.
(module
	(import "seal0" "call_runtime" (func $call_runtime (param i32 i32) (result i32)))
	(import "seal0" "seal_input" (func $seal_input (param i32 i32)))
	(import "seal0" "seal_return" (func $seal_return (param i32 i32 i32)))
    (import "seal0" "seal_debug_message" (func $seal_debug_message (param i32 i32) (result i32)))
	(import "env" "memory" (memory 1 1))

	;; 0x1000 = 4k in little endian
	;; size of input buffer
	(data (i32.const 0) "\00\10")

	(func $assert_eq (param i32 i32)
        (block $ok
            (br_if $ok
                (i32.eq (local.get 0) (local.get 1))
            )
            (unreachable)
        )
    )

	(func (export "call")
		;; Receive the encoded call
		(call $seal_input
			(i32.const 4)	;; Pointer to the input buffer
			(i32.const 0)	;; Size of the length buffer
		)

		;; Just use the call passed as input and store result to memory
		(i32.store (i32.const 0)
			(call $call_runtime
				(i32.const 4)				;; Pointer where the call is stored
				(i32.load (i32.const 0))	;; Size of the call
			)
		)

		(call $seal_return
			(i32.const 0)	;; flags
			(i32.const 0)	;; returned value
			(i32.const 4)	;; length of returned value
		)
	)

	(func (export "deploy"))
)