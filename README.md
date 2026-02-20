# git-id

A command-line tool to manage any number of GitHub accounts on one machine.

If you use several GitHub accounts, switching identities and remote URLs by hand gets messy quickly. `git-id` keeps every account registered in one place and switches the right identity and remote with a single command, no matter how many accounts you have.

Once `git-id` is on your `PATH`, git also recognises it as a subcommand, so you can type either form:

```
git-id use alice
git id use alice
```

---

## What it does

- Stores multiple GitHub accounts (username, email, SSH key or HTTPS token) in `~/.config/git-id/accounts.toml`
- Sets `user.name` and `user.email` in git config, either locally per-repo or globally
- Rewrites the `origin` remote URL to match the chosen account (SSH or HTTPS)
- Generates or registers `ed25519` SSH keys and writes the correct `~/.ssh/config` stanzas automatically
- Works with any git host: GitHub, GitLab, GitHub Enterprise, Gitea, and others

---

## Installation

Supports Linux and macOS on x86_64 and aarch64 (Apple Silicon).

```
curl -fsSL https://raw.githubusercontent.com/se7uh/git-id/main/install.sh | sh
```

The script downloads the correct pre-built binary from the latest release and places it in `~/.local/bin`. Make sure that directory is in your `PATH`.

To build from source instead:

```
cargo install --path .
```

---

## Usage

### Add an account

Run the interactive wizard once per account. It will ask for your username, email, remote type (SSH or HTTPS), and set up the key or token.

```
$ git-id add

  Add a new GitHub account

  GitHub username: alice
  Host [github.com]:
  Commit email: alice@example.com

  Remote type
  > ssh - use SSH keys (recommended)
    https - use personal access token
    both - configure SSH and HTTPS

  SSH Key
  > Generate new ed25519 key  (~/.ssh/id_ed25519_alice)
    Pick from existing ~/.ssh/*.pub keys

  Public key - paste this into GitHub -> Settings -> SSH keys:

  ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAA... alice@example.com

  Account 'alice@github.com' added!
  Next: git-id use alice   (inside a repo)  or  git-id use alice --global
```

---

### List accounts

```
$ git-id list

  Configured accounts  (4 total)

  alice  github.com  [active:local]
    email  : alice@example.com
    ssh    : ~/.ssh/id_ed25519_alice  priv:yes  pub:yes
    token  : -
    alias  : github.com-alice

  bob  github.com
    email  : bob@example.com
    ssh    : ~/.ssh/id_ed25519_bob  priv:yes  pub:yes
    token  : -
    alias  : github.com-bob

  carol  github.enterprise.io
    email  : carol@corp.io
    ssh    : ~/.ssh/id_ed25519_carol  priv:yes  pub:yes
    token  : -
    alias  : github.enterprise.io-carol

  dave  github.com  [active:global]
    email  : dave@example.org
    ssh    : ~/.ssh/id_ed25519_dave  priv:yes  pub:yes
    token  : -
    alias  : github.com-dave
```

---

### Switch identity inside a repository

Sets `user.name` and `user.email` locally and rewrites the `origin` remote URL to use the correct SSH alias.

```
$ cd ~/projects/my-repo
$ git-id use alice

  Git identity (local): alice <alice@example.com>
```

Force a specific remote format:

```
$ git-id use alice --ssh
$ git-id use alice --https
```

---

### Switch identity globally

Applies to all repos that do not have a local override.

```
$ git-id use dave --global

  Git identity (global): dave <dave@example.org>
```

---

### Check current status

Shows global identity, repo-local identity, origin remote, loaded SSH agent keys, and which configured account is currently active.

```
$ git-id status

  git-id status

  Global git identity
    name : dave
    email: dave@example.org

  Repo identity  (my-repo)
    name  : alice
    email : alice@example.com
    origin: git@github.com-alice:alice/my-repo.git

  ssh-agent keys
    OK 256 SHA256:abc123... alice@example.com (ED25519)

  Matched account: alice  github.com
```

---

### Remove an account

```
$ git-id remove alice

  About to remove account: alice  github.com
    email: alice@example.com
    key  : ~/.ssh/id_ed25519_alice

  Confirm removal? [y/N]: y

  Account 'alice@github.com' removed.
  SSH key files kept (use --delete-keys to also remove them)
```

Remove the SSH key files at the same time:

```
$ git-id remove alice --delete-keys -y
```

---

### SSH key management

Generate a new key for an existing account:

```
$ git-id ssh gen alice
```

Associate an existing `~/.ssh/*.pub` key with an account:

```
$ git-id ssh pick alice
```

Regenerate `~/.ssh/config` stanzas for all accounts:

```
$ git-id ssh config
```

---

### Dry run

Add `--dry-run` to any command to preview what would change without touching any files.

```
$ git-id use alice --dry-run

  [dry-run] git config --local user.name alice
  [dry-run] git config --local user.email alice@example.com
  [dry-run] git remote set-url origin git@github.com-alice:alice/my-repo.git
```

---

### Shell completions

```
$ git-id completions bash
$ git-id completions zsh
$ git-id completions fish
```

---

## Config file

Accounts are stored in `~/.config/git-id/accounts.toml`:

```toml
[[accounts]]
username    = "alice"
email       = "alice@example.com"
host        = "github.com"
ssh_key     = "~/.ssh/id_ed25519_alice"
https_token = ""

[[accounts]]
username    = "bob"
email       = "bob@example.com"
host        = "github.com"
ssh_key     = "~/.ssh/id_ed25519_bob"
https_token = ""

[[accounts]]
username    = "carol"
email       = "carol@corp.io"
host        = "github.enterprise.io"
ssh_key     = "~/.ssh/id_ed25519_carol"
https_token = ""

[[accounts]]
username    = "dave"
email       = "dave@example.org"
host        = "github.com"
ssh_key     = "~/.ssh/id_ed25519_dave"
https_token = ""
```

---

## Using multiple accounts across many repos

Each repo can have its own identity independent of the global setting. Once you have all accounts registered, switching is one command per repo and you never have to touch git config or SSH config by hand again.

```
$ cd ~/projects/repo-a  &&  git-id use alice
$ cd ~/projects/repo-b  &&  git-id use bob
$ cd ~/projects/repo-c  &&  git-id use carol
```

---

## License

MIT
