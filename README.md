# 🤠 wrangler

![Banner](/banner.png)

[![crates.io](https://meritbadge.herokuapp.com/wrangler)](https://crates.io/crates/wrangler) &nbsp;
[![npm](https://img.shields.io/npm/v/@cloudflare/wrangler.svg)](https://www.npmjs.com/package/@cloudflare/wrangler) &nbsp;
[![GitHub Actions - Test Status](https://github.com/cloudflare/wrangler/workflows/Tests/badge.svg)](https://github.com/cloudflare/wrangler/actions) &nbsp;
[![GitHub Actions - Linter Status](https://github.com/cloudflare/wrangler/workflows/Linters/badge.svg)](https://github.com/cloudflare/wrangler/actions) &nbsp;

`wrangler` is a CLI tool designed for folks who are interested in using [Cloudflare Workers](https://workers.cloudflare.com/).

![Wrangler Demo](/wrangler-demo.gif)

## Installation

You have many options to install wrangler!

### Install with `npm`

We strongly recommend you install `npm` with a Node version manager like [`nvm`](https://github.com/nvm-sh/nvm#installing-and-updating), which puts the global `node_modules` in your home directory to eliminate permissions issues with `npm install -g`. Distribution-packaged `npm` installs often use `/usr/lib/node_modules` (which is root) for globally installed `npm` packages, and running `npm install -g` as `root` prevents `wrangler` from installing properly.


Once you've installed `nvm` and configured your system to use the `nvm` managed node install, run:

```bash
npm i @cloudflare/wrangler -g
```

If you are running an ARM based system (eg Raspberry Pi, Pinebook) you'll need to use the `cargo` installation method listed below to build wrangler from source.

#### Specify binary location

In case you need `wrangler`'s npm installer to place the binary in a non-default location (such as when using `wrangler` in CI), you can use the following configuration options to specify an install location:

- Environment variable: `WRANGLER_INSTALL_PATH`
- NPM configuration: `wrangler_install_path`

#### Specify binary site URL

In case you need to store/mirror binaries on premise you will need to specify where wrangler should search for them by providing any of the following:

- Environment variable: `WRANGLER_BINARY_HOST`
- NPM configuration: `wrangler_binary_host`

### Install with `cargo`

```bash
cargo install wrangler
```

If you don't have `cargo` or `npm` installed, you will need to follow these [additional instructions](https://developers.cloudflare.com/workers/tooling/wrangler/install/).

### Install on Windows

[perl is an external dependency of crate openssl-sys](https://github.com/sfackler/rust-openssl/blob/b027f1603189919d5f63c6aff483243aaa188568/openssl/src/lib.rs#L11-L15). If installing wrangler with cargo, you will need to have perl installed. We've tested with [Strawberry Perl](https://www.perl.org/get.html). If you instead install perl via scoop, you may need to also run `scoop install openssl` in order to get the necessary openssl dlls. Installing wrangler with `npm` instead of cargo is another option if you don't want to install perl.

## Updating

For information regarding updating Wrangler, click [here](https://developers.cloudflare.com/workers/cli-wrangler/install-update#update).

## Getting Started

Once you have installed Wrangler, spinning up and deploying your first Worker is easy!

```console
$ wrangler generate my-worker
$ cd my-worker
# update your wrangler.toml with your Cloudflare Account ID
$ wrangler config
$ wrangler publish
```

## 🎙️ Top Level Commands

### 👯 `generate`

  Scaffold a project, including boilerplate code for a Rust library and a Cloudflare Worker.

  ```bash
  wrangler generate <name> <template> --type=["webpack", "javascript", "rust"]
  ```

  All of the arguments and flags to this command are optional:

  - `name`: defaults to `worker`
  - `template`: defaults to the [`https://github.com/cloudflare/worker-template`](https://github.com/cloudflare/worker-template)
  - `type`: defaults to `javascript` based on the ["worker-template"](https://github.com/cloudflare/worker-template/blob/master/wrangler.toml)

### 📥 `init`

  Creates a skeleton `wrangler.toml` in an existing directory. This can be used as an alternative to `generate` if you prefer to clone a repository yourself.

  ```bash
  wrangler init <name> --type=["webpack", "javascript", "rust"]
  ```

  All of the arguments and flags to this command are optional:

  - `name`: defaults to the name of your working directory
  - `type`: defaults to ["webpack"](https://developers.cloudflare.com/workers/tooling/wrangler/webpack).

### 🦀⚙️ `build`

  Build your project. This command looks at your `wrangler.toml` file and runs the build steps associated
  with the `"type"` declared there.

  Additionally, you can configure different [environments](https://developers.cloudflare.com/workers/tooling/wrangler/configuration/environments).


### 🔓 `login`

  Authenticate Wrangler with your Cloudflare login. This will prompt you with a Cloudflare account login page and is the alternative to `wrangler config`.

### 🔧 `config`

  Authenticate Wrangler with a Cloudflare API Token. This is an interactive command that will prompt you for your API token:

  ```bash
  wrangler config
  Enter API token:
  superlongapitoken
  ```

  You can also provide your email and global API key (this is not recommended for security reasons):

  ```bash
  wrangler config --api-key
  Enter email:
  testuser@example.com
  Enter global API key:
  superlongapikey
  ```

  You can also [use environment variables](https://developers.cloudflare.com/workers/tooling/wrangler/configuration/) to configure these values.

### ☁️ 🆙 `publish`

  Publish your Worker to Cloudflare. Several keys in your `wrangler.toml` determine whether you are publishing to a workers.dev subdomain or your own registered domain, proxied through Cloudflare.

  Additionally, you can configure different [environments](https://developers.cloudflare.com/workers/tooling/wrangler/configuration/environments).

  You can also use environment variables to handle authentication when you publish a Worker.

  ```bash
  # e.g.
  CF_API_TOKEN=superlongtoken wrangler publish
  # where
  # $CF_API_TOKEN -> your Cloudflare API token

  CF_API_KEY=superlongapikey CF_EMAIL=testuser@example.com wrangler publish
  # where
  # $CF_API_KEY -> your Cloudflare API key
  # $CF_EMAIL -> your Cloudflare account email
  ```

### 🗂 `kv`

  Interact with your Workers KV store. This is actually a whole suite of subcommands. Read more about in [Wrangler KV Documentation](https://developers.cloudflare.com/workers/tooling/wrangler/kv_commands).

### 👂 `dev`

  `wrangler dev` works very similarly to `wrangler preview` except that instead of opening your browser to preview your worker, it will start a server on localhost that will execute your worker on incoming HTTP requests. From there you can use cURL, Postman, your browser, or any other HTTP client to test the behavior of your worker before publishing it.

  You should run wrangler dev from your worker directory, and if your worker makes any requests to a backend, you should specify the host with `--host example.com`.

  From here you should be able to send HTTP requests to `localhost:8787` along with any headers and paths, and your worker should execute as expected. Additionally, you should see console.log messages and exceptions appearing in your terminal.

  ```bash
👂 Listening on http://localhost:8787
  [2020-02-18 19:37:08] GET example.com/ HTTP/1.1 200 OK
  ```

  All of the arguments and flags to this command are optional:

  - `env`: environment to build
  - `host`: domain to test behind your worker. defaults to example.com
  - `ip`: ip to listen on. defaults to localhost
  - `port`: port to listen on. defaults to 8787

### 🌎 `zone`

  Fetch the Cloudflare Account & Zone ID of the supplied domain name.
  
  You can use the returned ID as the `zone_id` field of your `wrangler.toml` configuration file to publish a Worker on a custom domain.

  ```bash
  $ wrangler zone example.com
  Zone information for example.com

  Zone ID: e405e53ee0fb1190c104316d5d306a19
  Account: Example Inc. (0f384009b201d49267977ca75b6b5918)
  Status: Active
  ```

## Additional Documentation

All information regarding wrangler or Cloudflare Workers is located in the [Cloudflare Workers Developer Docs](https://developers.cloudflare.com/workers/). This includes:

- Using wrangler [commands](https://developers.cloudflare.com/workers/tooling/wrangler/commands)
- Wrangler [configuration](https://developers.cloudflare.com/workers/tooling/wrangler/configuration)
- General documentation surrounding Workers development
- All wrangler features such as Workers Sites and KV

## ✨Workers Sites

To learn about deploying static assets using `wrangler`, see the [Workers Sites Quickstart](https://developers.cloudflare.com/workers/sites/).
