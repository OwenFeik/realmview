let
  pkgs = import <nixpkgs> {};
in
  pkgs.mkShell rec {
    buildInputs = with pkgs; [
      rustup
      wasm-pack
      sqlite
      gnumake

      (python3.withPackages (python-pkgs: [
        python-pkgs.urllib3
      ]))
    ];
  }
