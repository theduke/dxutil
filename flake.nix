{
  description = "Development environment for the dxutil Dioxus workspace";
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = inputs@{ flake-parts, rust-overlay, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      perSystem = { system, ... }:
        let
          pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          inherit (pkgs) lib;
          rust = pkgs.rust-bin.stable.latest.default.override {
            extensions = [ "clippy" "rustfmt" "rust-src" ];
            targets = [ "wasm32-unknown-unknown" ];
          };
          nativeLibraries = [ pkgs.openssl ] ++ lib.optionals pkgs.stdenv.isLinux
            (with pkgs; [ gtk3 webkitgtk_4_1 libsoup_3 xdotool librsvg ]);
          webdriver = pkgs.writeText "dxeditor-webdriver.json" (builtins.toJSON {
            "goog:chromeOptions" = {
              binary = "${pkgs.chromium}/bin/chromium";
              args = [ "--headless=new" "--no-sandbox" "--disable-dev-shm-usage" ];
            };
          });
          shell = pkgs.mkShell {
            packages = (with pkgs; [
              rust pkg-config nodejs_22 python3 gnumake git llvmPackages.clang llvmPackages.llvm
              dioxus-cli wasm-bindgen-cli binaryen actionlint shellcheck
            ]) ++ lib.optionals pkgs.stdenv.isLinux
              (with pkgs; [ chromium chromedriver playwright-driver.browsers ]);
            CC_wasm32_unknown_unknown = "${pkgs.llvmPackages.clang}/bin/clang";
            AR_wasm32_unknown_unknown = "${pkgs.llvmPackages.llvm}/bin/llvm-ar";
            buildInputs = nativeLibraries;
            LD_LIBRARY_PATH = lib.optionalString pkgs.stdenv.isLinux
              (lib.makeLibraryPath nativeLibraries);
            CHROMEDRIVER = lib.optionalString pkgs.stdenv.isLinux "${pkgs.chromedriver}/bin/chromedriver";
            WASM_BINDGEN_TEST_WEBDRIVER_JSON = lib.optionalString pkgs.stdenv.isLinux "${webdriver}";
            PLAYWRIGHT_BROWSERS_PATH = lib.optionalString pkgs.stdenv.isLinux "${pkgs.playwright-driver.browsers}";
            PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = lib.optionalString pkgs.stdenv.isLinux "1";
            PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = lib.optionalString pkgs.stdenv.isLinux "true";
            PLAYWRIGHT_HOST_PLATFORM_OVERRIDE = lib.optionalString pkgs.stdenv.isLinux "ubuntu-24.04";
          };
        in {
          devShells.default = shell;
          devShells.ui = shell;
          formatter = pkgs.nixfmt-rfc-style;
        };
    };
}
