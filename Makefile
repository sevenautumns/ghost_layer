FRAMEWORK = GhostLayer.xcframework
HEADERS   = include
TARGETS   = \
    aarch64-apple-ios        \
    aarch64-apple-ios-sim    \
    x86_64-apple-ios         \
    aarch64-apple-ios-macabi \
    x86_64-apple-ios-macabi  \
    aarch64-apple-darwin     \
    x86_64-apple-darwin

.PHONY: all apple lipo xcframework bindings zip checksum clean print-targets test \
        $(addprefix build-,$(TARGETS))

all: apple

bindings:
	mkdir -p $(HEADERS)
	cbindgen --lang c --cpp-compat -o $(HEADERS)/header.h
	cp GhostLayer.modulemap $(HEADERS)/module.modulemap

build-%:
	cargo build --release --target $*

lipo:
	lipo -create target/aarch64-apple-ios-sim/release/libghostlayer.a target/x86_64-apple-ios/release/libghostlayer.a -output target/libghostlayer-sim.a
	lipo -create target/aarch64-apple-ios-macabi/release/libghostlayer.a target/x86_64-apple-ios-macabi/release/libghostlayer.a -output target/libghostlayer-macabi.a
	lipo -create target/aarch64-apple-darwin/release/libghostlayer.a target/x86_64-apple-darwin/release/libghostlayer.a -output target/libghostlayer-macos.a

xcframework:
	rm -rf $(FRAMEWORK)
	xcodebuild -create-xcframework -library target/aarch64-apple-ios/release/libghostlayer.a -headers $(HEADERS) -library target/libghostlayer-sim.a -headers $(HEADERS) -library target/libghostlayer-macabi.a -headers $(HEADERS) -library target/libghostlayer-macos.a -headers $(HEADERS) -output $(FRAMEWORK)

zip:
	zip -r GhostLayer.xcframework.zip $(FRAMEWORK)

checksum:
	@swift package compute-checksum GhostLayer.xcframework.zip

apple: bindings $(addprefix build-,$(TARGETS))
	$(MAKE) lipo
	$(MAKE) xcframework

print-targets:
	@printf '%s\n' $(TARGETS)

test: apple
	@set -e; TMPDIR=$$(mktemp -d); trap "rm -rf $$TMPDIR" EXIT; cp -r Sources tests $(FRAMEWORK) "$$TMPDIR/"; cp Package.local.swift "$$TMPDIR/Package.swift"; DEVELOPER_DIR=$$(env -u DEVELOPER_DIR /usr/bin/xcode-select -p) SDKROOT=$$(env -u DEVELOPER_DIR -u SDKROOT /usr/bin/xcrun --sdk macosx --show-sdk-path) swift test --package-path "$$TMPDIR"

clean:
	rm -rf $(FRAMEWORK) $(HEADERS) GhostLayer.xcframework.zip \
		target/libghostlayer-sim.a target/libghostlayer-macabi.a target/libghostlayer-macos.a
