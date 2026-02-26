{
  description = "Rust Masscan scanner development shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        ccPath = "${pkgs.clang}/bin/clang";
        cxxPath = "${pkgs.clang}/bin/clang++";

        # On Darwin, `ld64.lld` currently rejects rustc's LTO plugin options.
        # Keep nix clang wrapper + default Apple linker for reliable SDK linking.
        rustFlags =
          if pkgs.stdenv.isDarwin then
            "-Clinker=clang"
          else
            "-Clinker-plugin-lto -Clinker=clang -Clink-arg=-fuse-ld=lld";

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
          ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            pkg-config
            cargo-deny
            cargo-nextest
            openssl

            masscan
            nmap
            libpcap
            tcpdump

            sqlite

            curl
            jq

            clang
            lld
            llvm
          ];

          env = {
            CC = ccPath;
            CXX = cxxPath;
            HOST_CC = ccPath;
            HOST_CXX = cxxPath;

            CC_aarch64_apple_darwin = ccPath;
            CXX_aarch64_apple_darwin = cxxPath;

            RUSTFLAGS = rustFlags;

            CRATE_CC_NO_DEFAULTS = if pkgs.stdenv.isDarwin then "1" else "0";

            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          };

          shellHook = ''
            set -e

            export CC="${ccPath}"
            export CXX="${cxxPath}"
            export HOST_CC="${ccPath}"
            export HOST_CXX="${cxxPath}"
            export CC_FOR_TARGET="${ccPath}"
            export CXX_FOR_TARGET="${cxxPath}"
            export CC_aarch64_apple_darwin="${ccPath}"
            export CXX_aarch64_apple_darwin="${cxxPath}"

            rust_llvm_version=$(rustc -vV | sed -n 's/^LLVM version: //p')
            clang_llvm_version=$(clang --version | sed -n 's/.*version \([0-9][0-9]*\.[0-9][0-9]*\(\.[0-9][0-9]*\)\?\).*/\1/p' | head -n 1)

            if [ -z "$rust_llvm_version" ] || [ -z "$clang_llvm_version" ]; then
              echo "error: failed to detect LLVM versions from rustc/clang"
              return 1
            fi

            if [ "$rust_llvm_major_minor" != "$clang_llvm_major_minor" ]; then
              echo "error: rustc LLVM ($rust_llvm_version) and clang LLVM ($clang_llvm_version) differ"
              return 1
            fi

            rustc --version
            clang --version | head -n 1
            ld.lld --version | head -n 1
            masscan --version || true
            nmap --version | head -n 1
            sqlite3 --version
            curl --version | head -n 1
          '';
        };
      }
    );
}
