# nix-rage

nix-rage is [age](https://github.com/FiloSottile/age)/[rage](https://github.com/str4d/rage) based tool designed to manage of encrypted configuration files within the Nix ecosystem.
Unlike [agenix](https://github.com/ryantm/agenix) or [sops-nix](https://github.com/Mic92/sops-nix), this tool is not designed for the secure use of passwords, tokens, etc. It is designed to hide personal information in public repositories. If you want to share your fancy nix config, but do not want to disclose your home address or your "secret" email, then this is the tool for you.

Strongly inspired by [oddlama's](https://github.com/oddlama) article ["Evaluation time secrets in Nix: Importing encrypted nix files"](https://oddlama.org/blog/evaluation-time-secrets-in-nix/).

> [!WARNING]  
> The `nix-rage` package is currently in an unstable development phase and is not recommended for use in sensitive configurations.

## Features

- **Seamless Integration**: Integrate encrypted configuration files directly within your Nix configuration.
- **Simplicity**: No need to preconfigure your repository with external tools (like git-crypt).
- **Security**: Securely manage sensitive configurations without exposing them in plaintext to public.

## Installation

You need to add plugin-files inside you `nix.conf` (`~/.config/nix/nix.conf`, `/etc/nix/nix.conf`):

```
# with nix-env:
plugin-files = /home/YOURUSERNAMEHERE/.nix-profile/lib/libnix_doc_plugin.so

# with cago build:
plugin-files = /path/to/repo/target/debug/libnix_rage.so

# inside nix config:
plugin-files = ${pkgs.nix-rage}/lib/libnix_rage.so
```

Nix Flake example:

```nix
{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    nix-rage.url = "github:renesat/nix-rage";
    nix-rage.inputs.nixpkgs.follows = "nixpkgs";
    #...
  };

  outputs = {self, nixpkgs, nix-rage, ..}: {
    nixosConfigurations = {
      myhostname = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          {
            nix.extraOptions = let
              nix-rage-package = nix-rage.packages."x86_64-linux".default;
            in ''
            plugin-files = ${nix-rage-package}/lib/libnix_rage.so
            '';
          }
          #...
        ];
      };
    };
  };
}
```

## Build From Source

Clone the repository and build nix-rage locally:

```bash
git clone https://github.com/renesat/nix-rage.git
cd nix-rage

# Using nix
nix build

# Using cargo
cargo build
```

## Usage

First create secret config:

`secret.nix`:
```nix
{
  mySecretEmail = "nagibator96@gmail.com"
  #...
}
```

Now we need to encrypt using `age`
`secret.nix`:
```bash
age --encrypt -r <AGE-KEY> secret.nix -o secret.nix.age
```

Now we can use this file in our config:

```nix
{...}:
let
  secrets = builtins.importAge [ ./secret-key ] ./secret.nix.age
in {
  some.config.parameters.email = secrets.mySecretEmail;
}
```

Also, you can read other files:

```nix
{...}:
let
  secretConfig = builtins.readAgeFile [ ./secret-key ] ./secret.toml.age
in {
  #...
}
```

## Contributing

Contributions are welcome! Feel free to open issues or submit pull requests on [GitHub](https://github.com/renesat/nix-rage).

## Related software

You might also be interested in:

- [git-crypt](https://github.com/AGWA/git-crypt)
- [git-agecrypt](https://github.com/vlaci/git-agecrypt)
- [agenix](https://github.com/ryantm/agenix)
- [sops-nix](https://github.com/Mic92/sops-nix)
- [agenix-rekey](https://github.com/oddlama/agenix-rekey)

## License

nix-rage is licensed under the MIT License. See the [LICENSE](LICENSE) file for more information.

