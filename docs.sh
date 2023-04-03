#!/bin/sh

cargo readme > README.md
(
	cd silly-alloc-macros
	cargo readme > README.md
)
