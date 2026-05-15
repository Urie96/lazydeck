# Nix environment and nix-build target for cross-compiling lazydeck to Termux/Android.
#
# Development shell:
#   nix-shell nix/android.nix -A passthru.shell
#   cargo build --release --target aarch64-linux-android
#
# Reproducible build:
#   nix-build nix/android.nix
#   ls -l result/bin/lazydeck
#
# Optional overrides:
#   nix-build nix/android.nix --argstr target aarch64-linux-android --argstr apiLevel 24
#   nix-shell nix/android.nix -A passthru.shell --argstr target x86_64-linux-android
#
# Notes:
# - This uses Android NDK from nixpkgs androidenv, not a locally installed NDK.
# - If the build fails in arboard/OpenSSL, apply the Android-specific dependency
#   changes discussed earlier: make arboard non-Android only, and use reqwest rustls.

{ target ? "aarch64-linux-android"
, apiLevel ? "24"
, ndkVersion ? "26.3.11579264"
, androidPlatformVersion ? "35"
, androidBuildToolsVersion ? "35.0.0"
, profile ? "release"
}:

let
  sources = import ./sources.nix;

  pkgs = import sources.nixpkgs {
    config = {
      allowUnfree = true;
      android_sdk.accept_license = true;
    };
    overlays = [ (import sources.rust-overlay) ];
  };

  targetInfo = {
    aarch64-linux-android = {
      clangPrefix = "aarch64-linux-android";
      cargoLinkerEnv = "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER";
      abi = "arm64-v8a";
    };
    armv7-linux-androideabi = {
      clangPrefix = "armv7a-linux-androideabi";
      cargoLinkerEnv = "CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER";
      abi = "armeabi-v7a";
    };
    x86_64-linux-android = {
      clangPrefix = "x86_64-linux-android";
      cargoLinkerEnv = "CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER";
      abi = "x86_64";
    };
    i686-linux-android = {
      clangPrefix = "i686-linux-android";
      cargoLinkerEnv = "CARGO_TARGET_I686_LINUX_ANDROID_LINKER";
      abi = "x86";
    };
  }.${target} or (throw "Unsupported Android Rust target: ${target}");

  androidPkgs = pkgs.androidenv.composeAndroidPackages {
    includeNDK = true;
    ndkVersions = [ ndkVersion ];
    platformVersions = [ androidPlatformVersion ];
    buildToolsVersions = [ androidBuildToolsVersion ];
    abiVersions = [ targetInfo.abi ];
  };

  androidSdk = androidPkgs.androidsdk;
  ndk = androidPkgs.ndk-bundle;

  rustToolchain = pkgs.rust-bin.stable.latest.default.override {
    extensions = [ "rust-src" "rustfmt" ];
    targets = [ target ];
  };

  rustPlatform = pkgs.makeRustPlatform {
    cargo = rustToolchain;
    rustc = rustToolchain;
  };

  hostTag =
    if pkgs.stdenv.hostPlatform.isDarwin then "darwin-x86_64"
    else if pkgs.stdenv.hostPlatform.isLinux then "linux-x86_64"
    else throw "Unsupported Nix host platform for Android NDK";

  ndkBin = "${ndk}/libexec/android-sdk/ndk/${ndkVersion}/toolchains/llvm/prebuilt/${hostTag}/bin";
  linker = "${ndkBin}/${targetInfo.clangPrefix}${apiLevel}-clang";
  cxx = "${ndkBin}/${targetInfo.clangPrefix}${apiLevel}-clang++";
  ar = "${ndkBin}/llvm-ar";
  builtinsLibName =
    if target == "aarch64-linux-android" then "libclang_rt.builtins-aarch64-android.a"
    else if target == "armv7-linux-androideabi" then "libclang_rt.builtins-arm-android.a"
    else if target == "x86_64-linux-android" then "libclang_rt.builtins-x86_64-android.a"
    else if target == "i686-linux-android" then "libclang_rt.builtins-i686-android.a"
    else throw "Unsupported Android Rust target: ${target}";
  clangBuiltins = "${ndkBin}/../lib/clang/17/lib/linux/${builtinsLibName}";

  isRelease = profile == "release";
  cargoProfileArgs = if isRelease then "--release" else "";
  cargoProfileDir = if isRelease then "release" else "debug";

  nativeCc = "${pkgs.stdenv.cc}/bin/cc";
  nativeCxx = "${pkgs.stdenv.cc}/bin/c++";

  commonEnv = {
    ANDROID_HOME = androidSdk;
    ANDROID_SDK_ROOT = androidSdk;
    ANDROID_NDK_HOME = "${ndk}/libexec/android-sdk/ndk/${ndkVersion}";
    ANDROID_NDK_ROOT = "${ndk}/libexec/android-sdk/ndk/${ndkVersion}";
    AR = ar;
    CC = linker;
    CXX = cxx;
    # LuaJIT builds and executes a small host tool (host/minilua) during
    # cross-compilation. Keep CC/CXX pointed at the Android compiler for target
    # C code, but force LuaJIT's HOST_CC to the native compiler; otherwise it
    # builds host/minilua as an Android binary and then cannot execute it on the
    # build machine.
    HOST_CC = nativeCc;
    HOST_CXX = nativeCxx;
    BUILD_CC = nativeCc;
    BUILD_CXX = nativeCxx;
    RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
  };
in
rustPlatform.buildRustPackage (commonEnv // {
  pname = "lazydeck-termux";
  version = "0.1.0";

  src = pkgs.lib.cleanSource ../.;

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  nativeBuildInputs = with pkgs; [
    pkg-config
    perl
    cmake
    which
    makeWrapper
  ];

  buildInputs = [ androidSdk ndk ];

  # Cross linker used by Cargo/rustc.
  "${targetInfo.cargoLinkerEnv}" = linker;

  # Build scripts that compile C/C++ code, such as vendored LuaJIT, should use NDK clang.
  preBuild = ''
    export PATH="${ndkBin}:$PATH"
    export AR="${ar}"
    export CC="${linker}"
    export CXX="${cxx}"
    export HOST_CC="${nativeCc}"
    export HOST_CXX="${nativeCxx}"
    export BUILD_CC="${nativeCc}"
    export BUILD_CXX="${nativeCxx}"
    export ${targetInfo.cargoLinkerEnv}="${linker}"
    export RUSTFLAGS="''${RUSTFLAGS:-} -C link-arg=${clangBuiltins}"

    echo "Android target: ${target}"
    echo "Android API level: ${apiLevel}"
    echo "NDK: $ANDROID_NDK_HOME"
    echo "Linker: ${linker}"
  '';

  buildPhase = ''
    runHook preBuild
    cargo build --frozen ${cargoProfileArgs} --target ${target}
    runHook postBuild
  '';

  doCheck = false;

  installPhase = ''
    runHook preInstall
    install -Dm755 target/${target}/${cargoProfileDir}/lazydeck $out/bin/lazydeck
    runHook postInstall
  '';

  meta = with pkgs.lib; {
    description = "lazydeck cross-compiled for Termux/Android";
    license = licenses.mit;
    platforms = platforms.linux ++ platforms.darwin;
  };

  passthru = {
    inherit androidSdk ndk rustToolchain target linker;
    shell = pkgs.mkShell (commonEnv // {
      packages = with pkgs; [
        rustToolchain
        androidSdk
        ndk
        pkg-config
        perl
        cmake
        which
        cargo-edit
        rust-analyzer
      ];

      shellHook = ''
        export PATH="${ndkBin}:$PATH"
        export ${targetInfo.cargoLinkerEnv}="${linker}"
        export RUSTFLAGS="''${RUSTFLAGS:-} -C link-arg=${clangBuiltins}"
        export AR="${ar}"
        export CC="${linker}"
        export CXX="${cxx}"
        export HOST_CC="${nativeCc}"
        export HOST_CXX="${nativeCxx}"
        export BUILD_CC="${nativeCc}"
        export BUILD_CXX="${nativeCxx}"

        echo "lazydeck Android cross environment"
        echo "  target:    ${target}"
        echo "  api level: ${apiLevel}"
        echo "  ndk:       $ANDROID_NDK_HOME"
        echo "  linker:    ${linker}"
        echo
        echo "Build manually with:"
        echo "  cargo build --release --target ${target}"
        echo
        echo "Or use nix-build:"
        echo "  nix-build nix/android.nix"
      '';
    });
  };
})
