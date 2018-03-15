# Rust-Callist - callisto network client

## [» Download the Source Code «](https://github.com/EthereumCommonwealth/rust-callisto)

### Join the chat!
Main site: https://callisto.network

Github: https://github.com/EthereumCommonwealth

Twitter: http://twitter.com/CallistoSupport

Reddit: http://reddit.com/r/CallistoCrypto

Facebook: https://www.facebook.com/callistonetwork

Discord: https://discord.gg/fGdPSA2

Telegram: https://t.me/CallistoNetwork


----

## About Rust-Callisto

Rust-Callisto comes with a built-in wallet. To access [Rust-Callisto Wallet](http://web3.site/) simply go to http://web3.site/ (if you don't have access to the internet, but still want to use the service, you can also use http://127.0.0.1:8180/). It includes various functionality allowing you to:

- create and manage your Callisto Network accounts;
- manage your Ether and any Ethereum tokens;
- create and register your own tokens;
- and much more.

By default, Rust-Callisto will also run a JSONRPC server on `127.0.0.1:8545` and a websockets server on `127.0.0.1:8546`. This is fully configurable and supports a number of APIs.

If you run into an issue while using Rust-Callisto, feel free to file one in this repository or hop on our [Discord](https://discord.gg/fGdPSA2) chat room to ask a question. We are glad to help!

**For security-critical issues**, please refer to the security policy outlined in [SECURITY.MD](SECURITY.md).

Rust-Callisto current release is CLO/1.0. You can download its source at https://github.com/EthereumCommonwealth/rust-callisto and follow the instructions below to build from source.

----

## Build dependencies

**Rust-Callisto requires Rust version 1.23.0 to build**

We recommend installing Rust through [rustup](https://www.rustup.rs/). If you don't already have rustup, you can install it like this:

- Linux:
	```bash
	$ curl https://sh.rustup.rs -sSf | sh
	```

	Rust-Callisto also requires `gcc`, `g++`, `libssl-dev`/`openssl`, `libudev-dev` and `pkg-config` packages to be installed.

- OSX:
	```bash
	$ curl https://sh.rustup.rs -sSf | sh
	```

	`clang` is required. It comes with Xcode command line tools or can be installed with homebrew.

- Windows
  Make sure you have Visual Studio 2015 with C++ support installed. Next, download and run the rustup installer from
	https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe, start "VS2015 x64 Native Tools Command Prompt", and use the following command to install and set up the msvc toolchain:
  ```bash
	$ rustup default stable-x86_64-pc-windows-msvc
  ```

Once you have rustup, install Parity or download and build from source

----


## Build from source

```bash
# download Rust-Callisto code
$ git clone -b CLO/1.0 https://github.com/EthereumCommonwealth/rust-callisto.git
$ cd rust-callisto

# build in release mode
$ cargo build --release
```

This will produce an executable in the `./target/release` subdirectory.

Note: if cargo fails to parse manifest try:

```bash
$ ~/.cargo/bin/cargo build --release
```

Note: When compiling a crate and you receive the following error:

```
error: the crate is compiled with the panic strategy `abort` which is incompatible with this crate's strategy of `unwind`
```

Cleaning the repository will most likely solve the issue, try:

```bash
$ cargo clean
```

This will always compile the latest nightly builds. If you want to build stable or beta, do a

```bash
$ git checkout stable
```

or

```bash
$ git checkout beta
```

first.

----


## Start Rust-Callisto

### Manually

To start Parity manually, just run

```bash
$ ./target/release/parity
```

and Parity will begin syncing the Ethereum blockchain.
