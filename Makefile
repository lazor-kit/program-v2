.PHONY: build build-and-sync clean

# Default target - build and sync
build-and-sync:
	@echo "🚀 Building and syncing LazorKit..."
	anchor build
	@echo "🔄 Syncing IDL and types to contract-integration..."
	cp target/idl/lazorkit.json contract-integration/anchor/idl/lazorkit.json
	cp target/idl/default_policy.json contract-integration/anchor/idl/default_policy.json
	cp target/types/lazorkit.ts contract-integration/anchor/types/lazorkit.ts
	cp target/types/default_policy.ts contract-integration/anchor/types/default_policy.ts
	@echo "✅ Build and sync complete!"

# Just build (no sync)
build:
	anchor build

init-idl:
	anchor idl init -f ./target/idl/lazorkit.json Gsuz7YcA5sbMGVRXT3xSYhJBessW4xFC4xYsihNCqMFh
	anchor idl init -f ./target/idl/default_policy.json BiE9vSdz9MidUiyjVYsu3PG4C1fbPZ8CVPADA9jRfXw7

upgrade-idl:
	anchor idl upgrade Gsuz7YcA5sbMGVRXT3xSYhJBessW4xFC4xYsihNCqMFh -f ./target/idl/lazorkit.json
	anchor idl upgrade BiE9vSdz9MidUiyjVYsu3PG4C1fbPZ8CVPADA9jRfXw7 -f ./target/idl/default_policy.json

deploy: 
	anchor deploy

# Clean build artifacts
clean:
	anchor clean