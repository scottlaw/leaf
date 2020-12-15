ios:
	cargo lipo -p leaf-mobile --release --targets aarch64-apple-ios --manifest-path leaf-mobile/Cargo.toml
	cbindgen leaf-mobile/src/lib.rs -l c > target/universal/release/leaf.h

ios-x86:
	cargo lipo -p leaf-mobile --release --targets x86_64-apple-ios --manifest-path leaf-mobile/Cargo.toml
	cbindgen leaf-mobile/src/lib.rs -l c > target/universal/release/leaf.h

ios-universal:
	# arm64
	cargo lipo -p leaf-mobile --release --targets aarch64-apple-ios --manifest-path leaf-mobile/Cargo.toml
	# x86
	cargo lipo -p leaf-mobile --release --targets x86_64-apple-ios --manifest-path leaf-mobile/Cargo.toml
	# header
	cbindgen leaf-mobile/src/lib.rs -l c > target/universal/release/leaf.h
	# create fat library
	lipo -create target/x86_64-apple-ios/release/libleaf.a  target/aarch64-apple-ios/release/libleaf.a -o target/universal/release/libleaf.a

ios-dev:
	cargo lipo -p leaf-mobile --targets aarch64-apple-ios --manifest-path leaf-mobile/Cargo.toml
	cbindgen leaf-mobile/src/lib.rs -l c > target/universal/debug/leaf.h

local:
	cargo build -p leaf-bin --release

local-dev:
	cargo build -p leaf-bin

# Force a re-generation of protobuf files.
proto-gen:
	touch leaf/build.rs
	PROTO_GEN=1 cargo build -p leaf
