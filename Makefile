FRAMEWORK  = GhostLayer.xcframework
HEADERS    = include

.PHONY: apple clean

apple:
	cargo build --release --target aarch64-apple-ios
	cargo build --release --target aarch64-apple-ios-sim
	cargo build --release --target x86_64-apple-ios
	cargo build --release --target aarch64-apple-ios-macabi
	cargo build --release --target x86_64-apple-ios-macabi
	cargo build --release --target aarch64-apple-darwin
	cargo build --release --target x86_64-apple-darwin
	lipo -create \
		target/aarch64-apple-ios-sim/release/libghostlayer.a \
		target/x86_64-apple-ios/release/libghostlayer.a \
		-output target/libghostlayer-sim.a
	lipo -create \
		target/aarch64-apple-ios-macabi/release/libghostlayer.a \
		target/x86_64-apple-ios-macabi/release/libghostlayer.a \
		-output target/libghostlayer-macabi.a
	lipo -create \
		target/aarch64-apple-darwin/release/libghostlayer.a \
		target/x86_64-apple-darwin/release/libghostlayer.a \
		-output target/libghostlayer-macos.a
	mkdir -p $(HEADERS)
	cp header.h $(HEADERS)/
	xcodebuild -create-xcframework \
		-library target/aarch64-apple-ios/release/libghostlayer.a \
		-headers $(HEADERS) \
		-library target/libghostlayer-sim.a \
		-headers $(HEADERS) \
		-library target/libghostlayer-macabi.a \
		-headers $(HEADERS) \
		-library target/libghostlayer-macos.a \
		-headers $(HEADERS) \
		-output $(FRAMEWORK)

clean:
	rm -rf $(FRAMEWORK) $(HEADERS) \
		target/libghostlayer-sim.a target/libghostlayer-macabi.a target/libghostlayer-macos.a
