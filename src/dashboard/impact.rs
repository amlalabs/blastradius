//! Curated "why it's risky" / "what an agent can do" copy for the dashboard.
//!
//! This is teaching copy for the finding-impact explanations the dashboard
//! renders next to each reachable finding. It is value-free generic prose
//! (no secret values, no machine-specific data): a per-finding-id map, a
//! per-`FindingClass` fallback for ids without bespoke copy, and a one-line
//! "why" for each session signal. The dashboard prefers the per-id copy and
//! falls back to the class copy by `Finding::class.to_string()`.

/// Curated (why, how) for a specific finding id, verbatim. `None` when the id
/// has no bespoke copy — callers fall back to [`finding_impact_class`].
pub fn finding_impact(id: &str) -> Option<(&'static str, &'static str)> {
    let pair = match id {
        "aws.credentials.profiles" => (
            "A reachable AWS credentials file means an agent holds your standing keys to whatever those profiles can touch in AWS — often production accounts — with the same blast radius as your own console access.",
            "Code running as you can use any listed profile (default, prod, etc.) to call the AWS API: read S3 buckets, query databases, spin up or delete infrastructure, and pivot through IAM to anything those roles permit, all without any further authentication.",
        ),
        "aws.sso_cache" => (
            "The AWS profile probe only reads names, but the SSO/CLI token cache holds LIVE bearer tokens — already-issued, ready-to-use cloud credentials — so a reachable cache is an active session, not just a config reference.",
            "An agent can lift a cached SSO/STS token and make authenticated AWS calls as your current session until it expires, reaching every account and role your SSO login was granted without prompting you to re-authenticate.",
        ),
        "azure.credentials" => (
            "A reachable Azure token cache (MSAL/access tokens, service-principal entries) is a live or near-live login to your Azure tenant, carrying whatever subscriptions and roles your identity holds.",
            "Code running as you can use the cached tokens or service-principal secret to call Azure Resource Manager — enumerate and modify resources, read Key Vault secrets, or manage subscriptions — up to the scope of your account.",
        ),
        "gcp.credentials" => (
            "The gcloud config holds application-default credentials and stored refresh tokens — long-lived keys that mint fresh access tokens — so reachability means durable, renewable access to your GCP projects rather than a single expiring session.",
            "An agent can use the refresh/ADC credentials to obtain GCP access tokens and act as your account or service account: read Cloud Storage, query BigQuery, change IAM, or manage compute across every project your identity can reach.",
        ),
        "kube.config" => (
            "A reachable kubeconfig often carries cluster-admin or broadly-scoped contexts, and a single file can hold credentials to many clusters at once, so it is frequently the highest-leverage credential on a developer's machine.",
            "Code running as you can run any kubectl/API action your contexts allow — read every Secret in the cluster, exec into running pods, deploy or delete workloads, and in cluster-admin contexts take over the whole cluster — against production clusters if those contexts are present.",
        ),
        "kube.pod_token" => (
            "When the agent runs inside a Kubernetes pod, the mounted service-account token is a live cluster credential issued to that workload, present and reachable by anything in the pod.",
            "Code running as you can present this token to the Kubernetes API and do whatever the pod's ServiceAccount permits — read Secrets, talk to other services, or, if the SA is over-privileged, escalate toward broader cluster access.",
        ),
        "docker.registry_auth" => (
            "A reachable ~/.docker/config.json commonly stores registry auth as base64 (effectively plaintext) credentials, giving direct access to the image registries you publish and pull from.",
            "An agent can read the registry tokens and push or pull images as you — for example poisoning a published image with malicious layers or pulling private images — turning your registry identity into a supply-chain foothold.",
        ),
        "container.registry_auth" => (
            "Podman, skopeo, and buildah keep their own auth.json (under ~/.config/containers or XDG_RUNTIME_DIR) separate from Docker's, so this credential store can be reachable even when the Docker one is locked down.",
            "Code running as you can use the stored registry tokens to push or pull container images as you, including overwriting published images in private registries, with the same supply-chain consequences as the Docker credential.",
        ),
        "container.runtime_secrets" => (
            "Secrets mounted at /run/secrets (Docker/Swarm/compose) are readable by anything in the container, so when an agent runs inside that container the application's injected secrets are directly on disk.",
            "An agent can read every file under /run/secrets — database passwords, API keys, TLS keys handed to the workload — and use them to reach whatever backing services the containerized app talks to.",
        ),
        "databricks.cfg" => (
            "~/.databrickscfg stores workspace tokens in plaintext, and those tokens carry your access to data, notebooks, jobs, and compute in the Databricks workspace.",
            "Code running as you can use the token to call the Databricks API: read or export data from tables, run arbitrary jobs/notebooks on your clusters, and manage workspace resources as your identity.",
        ),
        "dbt.profiles" => (
            "~/.dbt/profiles.yml holds the warehouse connection credentials dbt uses — typically a database password or key with broad query rights to your analytics warehouse.",
            "An agent can read the warehouse credentials and connect directly (Snowflake, BigQuery, Redshift, Postgres, etc.) to read or modify production data tables outside of dbt entirely, with whatever rights the configured role holds.",
        ),
        "snowflake.config" => (
            "SnowSQL config and connections.toml store Snowflake connection details — often a password or key-pair — granting access to the data and compute in your Snowflake account.",
            "Code running as you can use the stored connection to query or alter Snowflake data, run warehouses (incurring cost), and reach every database and schema the configured role can see.",
        ),
        "terraform.token" => (
            "A reachable Terraform Cloud/Enterprise token represents control over your infrastructure-as-code: the state, variables (including secrets), and the ability to drive applies that change real cloud resources.",
            "An agent can use the token to read remote state and workspace variables (which frequently contain cloud credentials) and to queue plans/applies that create, modify, or destroy production infrastructure through Terraform Cloud.",
        ),
        "cloudflared.tunnel" => (
            "A cloudflared cert/credential lets the holder stand up Cloudflare Tunnels, which create inbound paths from the internet into your private network or services.",
            "Code running as you can use the credential to register and run tunnels — exposing internal services to the public internet or routing traffic into your network — turning a local file into a remote-access channel an attacker can reuse.",
        ),
        "rclone.config" => (
            "rclone.conf stores credentials for cloud-storage remotes, and rclone only lightly obscures (does not securely encrypt) them, so a reachable config is effectively plaintext access to every configured remote.",
            "An agent can deobfuscate the stored secrets and use rclone to read, copy out, or delete data across your configured S3/GCS/Drive/Dropbox and other remotes, enabling bulk exfiltration or destruction of stored data.",
        ),
        "vault.token" => (
            "~/.vault-token is a live HashiCorp Vault session token; reachability means an agent inherits your current Vault authentication and whatever policies it carries.",
            "Code running as you can present the token to Vault and read every secret your policies permit — database creds, cloud keys, certificates — and request dynamic secrets, effectively fanning out from one token to many downstream systems.",
        ),
        "cloud_init.user_data" => (
            "On cloud VMs, cloud-init user-data frequently embeds bootstrap secrets in plaintext (passwords, tokens, join keys), and the file persists on disk after first boot.",
            "An agent can read /var/lib/cloud/instance/user-data and harvest any provisioning secrets baked in — initial admin passwords, API tokens, or cluster join credentials — to reach the systems those secrets were meant to configure.",
        ),
        "cloud_legacy.config" => (
            "Legacy tool configs (~/.s3cfg, ~/.boto, ~/.dockercfg) store cloud and registry credentials in plaintext at the home level, and they are easy to forget when locking down newer credential paths.",
            "Code running as you can read these plaintext files to obtain AWS/S3 keys and registry tokens, then access object storage or push/pull images as you — the same reach as the modern credentials, via an overlooked older path.",
        ),
        "conda.tokens" => (
            "Anaconda upload tokens and authenticated channel URLs in the conda config grant publish and private-access rights to your conda channels, which feed your environments and possibly your team's.",
            "An agent can use the upload token to publish or overwrite packages on your Anaconda channels (a supply-chain risk for anyone installing from them) and read any private channels the authed URLs unlock.",
        ),
        "teleport.tsh" => (
            "The ~/.tsh key store holds Teleport-issued certificates that broker access to SSH hosts, Kubernetes clusters, databases, and applications behind Teleport, so one store fans out to many protected systems.",
            "Code running as you can use the tsh certificates to connect to any SSH server, cluster, or database your Teleport roles allow until the certs' short TTL expires — reaching production infrastructure that Teleport is meant to gate.",
        ),
        "cargo.token" => (
            "This file holds your crates.io publish token, a standing credential the agent can read and reuse without any further prompt.",
            "An agent can publish or yank crates under your account, including pushing a malicious version of a crate you own that other projects then pull in.",
        ),
        "npm.token" => (
            "Your ~/.npmrc auth token is a long-lived npm publish credential tied to your account and any packages or org you can write to.",
            "An agent can publish new package versions under your name, letting it slip backdoored code into a package that downstream installs trust and run.",
        ),
        "pypi.token" => (
            "~/.pypirc stores your PyPI upload token, a standing credential for releasing packages under your account.",
            "An agent can upload new releases of your PyPI projects, planting malicious code that anyone who pip-installs them will execute.",
        ),
        "pip.config" => (
            "A pip.conf with credentials baked into the index URL exposes a private package-index login in plaintext.",
            "An agent can read your private index credentials and use them to pull from, or publish poisoned packages to, your internal package repository.",
        ),
        "composer.auth" => (
            "Composer's auth.json holds tokens for the PHP package registries and private repos you authenticate to.",
            "An agent can use these tokens to read your private Composer packages or publish tampered versions that your PHP projects will pull in on the next install.",
        ),
        "gradle.credentials" => (
            "gradle.properties commonly stores publish tokens and signing-key passwords used to release Java/Android artifacts.",
            "An agent can read these to publish artifacts to your Maven/Gradle repositories or unlock a signing key, producing builds that downstream consumers trust as genuinely yours.",
        ),
        "maven.credentials" => (
            "~/.m2/settings.xml stores repository server passwords, often in plaintext, for the artifact repositories you deploy to.",
            "An agent can use these credentials to download your private artifacts or deploy tampered versions to your Maven repository that other builds will resolve.",
        ),
        "nuget.config" => (
            "NuGet.Config can contain package-source API keys and passwords (sometimes as ClearTextPassword) for your .NET feeds.",
            "An agent can read these to pull from your private NuGet feeds or push malicious package versions that your .NET projects restore and run.",
        ),
        "rubygems.credentials" => (
            "~/.gem/credentials and ~/.bundle/config hold RubyGems API keys and bundler auth for private gem sources.",
            "An agent can publish gems under your RubyGems account or pull from private gem servers, slipping backdoored gems into projects that bundle them.",
        ),
        "sops.age_keys" => (
            "This is an age/SOPS private decryption key, the master that unlocks every secret encrypted to it across your repos.",
            "An agent can decrypt all SOPS-encrypted files it can reach, turning your committed encrypted config into cleartext database passwords, API keys, and cloud credentials.",
        ),
        "ansible.vault_password" => (
            "This file is the Ansible Vault password that decrypts every vaulted secret in your playbooks and inventories.",
            "An agent can decrypt all vault-encrypted variables it can find, exposing the production passwords, keys, and tokens your automation injects into servers.",
        ),
        "mysql.client" => (
            "~/.my.cnf and ~/.mylogin.cnf store MySQL connection passwords (mylogin.cnf's obfuscation is trivially reversible), saved so the client logs in without prompting.",
            "An agent can connect to the databases these credentials point at and read, modify, or drop your data, or dump entire tables of user records.",
        ),
        "postgres.pgpass" => (
            "~/.pgpass stores Postgres passwords per host so connections succeed without a prompt, making them directly usable by anything running as you.",
            "An agent can connect to the listed Postgres servers and read or alter your data, including exporting whole databases of sensitive records.",
        ),
        "mail.credentials" => (
            "Mail client rc files (msmtp, authinfo, fetchmail, mbsync, etc.) store your SMTP/IMAP account passwords in plaintext.",
            "An agent can send email as you or read your inbox, enabling convincing phishing from your real address and harvesting password-reset links and other secrets from your mail.",
        ),
        "vpn.credentials" => (
            "WireGuard configs embed the interface private key and Tailscale state holds your node key, each a credential that places a device on your private network.",
            "An agent can bring up the tunnel and reach internal hosts and services that are firewalled off from the public internet, expanding far beyond the local machine.",
        ),
        "saas_cli.tokens" => (
            "These per-CLI token files (DigitalOcean, Fly, Vercel, Netlify, Supabase, Cloudflare, and similar) are standing API credentials for the services you deploy to.",
            "An agent can act on those accounts as you: redeploy or take down apps, change DNS or environment config, and read project data through each provider's API.",
        ),
        "onepassword.cli" => (
            "A live `op` session means the 1Password CLI is unlocked, so vault access is available without re-entering your master password.",
            "An agent can retrieve any item in your 1Password vaults, effectively handing it every password, API key, and recovery code you store there.",
        ),
        "password_manager.cli" => (
            "An unlocked password-manager CLI session or store (Bitwarden, LastPass, pass) is a single point that fronts most of your other credentials.",
            "An agent can dump the contents of your password store, exposing the logins, keys, and secrets for everything you keep in your password manager at once.",
        ),
        "ssh.private_keys" => (
            "Private SSH keys are a long-lived master credential: anything that can read the key file can authenticate as you to every host and Git remote that trusts it, with no further prompt.",
            "An agent can copy your readable private keys out of ~/.ssh and use them from anywhere to log into servers, push to Git over SSH, or reach bastions and prod hosts as you. Passphrase status is not checked, so an unencrypted key is immediately usable; an encrypted one is still exfiltratable for offline cracking.",
        ),
        "ssh.agent_socket" => (
            "A reachable ssh-agent socket lets code authenticate with every identity already loaded in the agent without ever touching the key files — including passphrase-protected keys whose on-disk form is useless to an attacker.",
            "An agent can connect to SSH_AUTH_SOCK and ask the agent to sign authentication challenges, effectively logging into any host or Git remote those loaded keys trust, even keys it could never decrypt on disk. Agent forwarding extends this to whatever the socket reaches.",
        ),
        "gpg.private_keys" => (
            "GPG secret keys are your cryptographic identity for signing and decryption; a reachable secret key undermines the trust that commit/tag/release signatures and encrypted files are supposed to provide.",
            "An agent can read your GPG secret keyring and forge signed Git commits, tags, or release artifacts that appear to come from you, and decrypt anything previously encrypted to your key. If the key is unprotected or its passphrase is cached, this needs no further input.",
        ),
        "gpg.agent_socket" => (
            "A reachable gpg-agent socket means signing and decryption can happen through the agent using any cached passphrase, so the protection of an encrypted key on disk is bypassed entirely.",
            "While a passphrase is cached in the agent, an agent can ask gpg-agent to sign or decrypt as you without the passphrase or the raw key file, producing forged signed commits/artifacts or reading encrypted data. A short default-cache-ttl shrinks this window.",
        ),
        "github.token_source" => (
            "A local GitHub auth source (gh CLI host config or a token-bearing env var) is a standing credential to your GitHub account and its repositories; this probe reads it locally and never contacts GitHub, so scope is unverified.",
            "An agent can read the stored GitHub token and act as you against the GitHub API and Git remotes — clone private repos, push code, open or merge pull requests, and depending on scope edit Actions, secrets, or org settings. Token scopes are not checked, so treat its reach as whatever that token was granted.",
        ),
        "git.credential_store" => (
            "Plaintext Git credential stores (~/.git-credentials, ~/.netrc) hold reusable usernames and passwords/tokens for remotes in a file with no unlock step, so any same-user read yields working credentials.",
            "An agent can read these files and reuse the stored host credentials to authenticate to GitHub, GitLab, package indexes, or other services over HTTPS — pushing code or calling APIs as you. A configured credential helper alone is lower-signal, but a plaintext store is directly usable.",
        ),
        "keyring.secret_store" => (
            "The OS keyring (GNOME Keyring/libsecret, KWallet, macOS login keychain) is the shared vault many tools use via the Secret Service/Keychain API; when it is unlocked, everything stored in it is reachable through normal APIs.",
            "With the keyring unlocked in your session, an agent can ask the Secret Service/Keychain for stored items and pull out saved passwords, tokens, and app credentials for many tools at once. Running agents in a separate session, or keeping the keyring locked, removes this.",
        ),
        "browser.session_stores" => (
            "Browser cookie jars hold live session tokens for every site you are logged into, and the Login Data DB holds saved passwords; a stolen session cookie reproduces an authenticated session and bypasses both the password and MFA.",
            "An agent can copy your browser cookie and saved-password databases and replay your active sessions to email, cloud consoles, banking, and SaaS dashboards as a logged-in you — without triggering a login or a second factor. On most platforms the decryption key is reachable to the same user too, so OS encryption is not a barrier.",
        ),
        "jupyter.runtime" => (
            "A running Jupyter server is authenticated by a runtime token, and that token grants the ability to execute arbitrary code in the notebook kernel — effectively local code execution under your identity.",
            "An agent can read a live server's runtime token from ~/.local/share/jupyter/runtime and connect to that notebook server to run arbitrary Python (or shell) in the kernel, reaching whatever data and credentials that kernel can. Don't leave notebook servers running where an agent can reach their runtime files.",
        ),
        "ai_assistant.credentials" => (
            "Coding assistants store their own auth tokens on disk (Claude, Copilot, Codeium, Cursor); those credential files are themselves a reachable secret, and an agent reading a sibling assistant's token can impersonate it.",
            "An agent can read these credential files and reuse the tokens to call the assistant's backend or account as you — or quietly exfiltrate them. Because they sit in predictable home paths, an agent with home access can harvest the credentials of whatever assistants are installed.",
        ),
        "env.secret_names" => (
            "Secret-named environment variables (API keys, tokens, DATABASE_URL, etc.) are inherited by every process the agent spawns and are among the easiest credentials to reach — no file read or unlock required, just reading its own environment.",
            "An agent already has these values in its own process environment and can use them directly: calling cloud, GitHub, OpenAI/Anthropic, Stripe, or database endpoints as you, or passing them onward. This probe reports only the variable names and value lengths, never the values.",
        ),
        "env.subprocess_scrub" => (
            "Even when the harness strips Anthropic/cloud credentials before spawning subprocesses, third-party tokens (GitHub, npm, Stripe, DATABASE_URL, etc.) are not covered by that scrub and still flow into every Bash, hook, and MCP subprocess.",
            "Any command the agent runs inherits these un-scrubbed credentials in its environment, so a subprocess (including one from an untrusted dependency or script) can read and use them as you. This finding names which present credentials survive the scrub; whether the scrub is on is derived from a flag and the covered list is docs-derived, not guaranteed.",
        ),
        "credentials.shell_history" => (
            "Shell history files persistently record commands you typed, and exported secrets, inline tokens, and credential-bearing URLs routinely end up there in plaintext that any same-user read recovers.",
            "An agent can read your zsh/bash/fish history and lift tokens and connection strings that were typed on the command line, then reuse them against the matching services. This probe only counts secret-looking lines per file and never prints the lines or matched substrings.",
        ),
        "credentials.repl_history" => (
            "Database clients and language REPLs keep their own history files (~/.psql_history, ~/.mysql_history, ~/.python_history, ~/.node_repl_history, etc.) that capture connection strings, \\password commands, and inline tokens — exactly as readable as shell history and almost never considered.",
            "An agent can read these REPL/DB-client histories and recover database connection URIs with embedded passwords or pasted tokens, then connect to those databases and services as you. Like the shell-history probe, it reports only per-file, per-category counts, never the lines themselves.",
        ),
        "atuin.sync" => (
            "The Atuin sync key decrypts your synced shell history — which commonly contains secrets — so a reachable key turns into access to your full cross-machine command history, not just the local file.",
            "An agent can read the Atuin key/session and decrypt or pull your synced shell history from the sync server, recovering tokens and connection strings typed on any of your machines. Keeping the Atuin key out of agent scope limits exposure to whatever is local.",
        ),
        "cross_repo.sibling_repos" => (
            "A coding agent's reach is set by your user identity, not by the one repo it was launched in: every other git checkout under your home is equally readable, so a task scoped to one project can quietly touch all of them.",
            "Code running as you can list and read the source, configs, and local files of every sibling repository found near your working tree, pulling in proprietary code, internal infrastructure definitions, and any secrets those projects keep on disk from a single starting point.",
        ),
        "cross_repo.lateral_secrets" => (
            "Secrets stored in your other checkouts are not protected by the boundary of the current project; they sit in plain files that any process running as you can open, turning one repo's agent into a key to many.",
            "An agent can read the .env files, private keys (.pem/.key/id_rsa), and service-account JSON sitting in neighboring repositories and use those credentials to reach the databases, cloud accounts, and third-party APIs of unrelated projects.",
        ),
        "cross_repo.dotenv.current" => (
            ".env files hold the live application secrets a service runs with, and they sit unencrypted in the working tree the agent already has open, so they are among the first credentials within reach.",
            "Code running as you can read every key and value in the current repo's .env files and use those database URLs, API tokens, and signing keys to authenticate to the project's real backends and external services.",
        ),
        "cross_repo.dotenv.siblings" => (
            "Other projects' .env files are just as readable as the current one, so an agent's effective credential set is the union of every checkout's secrets, not only the repo it was asked to work in.",
            "An agent can read the application secrets of unrelated sibling projects from their .env files and use those tokens and connection strings to reach systems that have nothing to do with the task it was given.",
        ),
        "git.push_likelihood" => (
            "If working git credentials (an SSH key, a gh/GitHub token, a credential-store or .netrc entry) match this repo's remote, code running as you can publish commits, so changes an agent makes are not confined to your local disk.",
            "An agent can commit and push to the remote, altering shared branches, CI/CD pipelines, and release artifacts that teammates and deploy systems trust; whether a specific branch is protected is enforced server-side and is not verified locally.",
        ),
        "git.config_exec_directives.local" => (
            "This repo's git config already contains directives that run a command or redirect a transport during ordinary git operations, so simply running git here executes configured code outside any Bash sandbox.",
            "Shell-executing aliases, core.sshCommand/fsmonitor, content filters, diff.external, or insteadOf URL rewrites in the repo's .git/config fire under your normal environment on routine git commands, letting attacker-planted config run commands or reroute fetch/push as you without an explicit shell invocation.",
        ),
        "egress.connectivity" => (
            "Anything an agent reads is only damaging off-machine if it can leave, and an open outbound path means there is no network boundary stopping data from being sent out.",
            "Code running as you can open outbound TLS connections to the internet, which is the channel needed to exfiltrate the credentials, source, and secrets found by the other probes or to pull down additional tooling.",
        ),
        "egress.mediation" => (
            "Whether outbound traffic is forced through a filtering proxy determines if egress is actually constrained; a hostname-only allowlist or direct routing leaves real exfiltration paths open, and reachable cloud metadata is a credential source of its own.",
            "With no proxy or a proxy that filters on hostname/SNI only (no TLS inspection), an agent can still exfiltrate to any allowed domain or via domain-fronting, and if the link-local metadata endpoint (169.254.169.254) is reachable it can fetch short-lived IMDS cloud credentials directly.",
        ),
        "host.privilege_escalation" => (
            "A coding agent running as you inherits any standing path you have to root, turning a single misstep from a user-level mistake into full host compromise. Passwordless sudo or membership in root-equivalent groups (docker, lxd, libvirt, kvm) means the leap to root needs no exploit and no password prompt.",
            "Code running as you can become root non-interactively and then read, modify, or destroy anything on the machine - other users' files, system binaries, the secret stores other probes flag - or install a permanent backdoor. With docker/lxd group access it can launch a container that mounts the host filesystem and edits root-owned files directly.",
        ),
        "host.privileged_reachability" => (
            "This is the blast radius IF the agent reaches root - via an escalation path on this host or a kernel exploit (the Claude Code sandbox runs default-allow seccomp, so the local-privilege-escalation surface is not contained). It shows how much sensitive system state sits one escalation away, and flags any root-owned file already readable today.",
            "Once root is reached, the agent can read every high-value system target inventoried here - the shadow password hashes, machine keys, every user's home and credentials. Any file shown as already readable (for example a group-readable /etc/shadow) is reachable right now with no escalation at all.",
        ),
        "process.afunix_docker_sock" => (
            "A reachable Docker daemon socket is equivalent to unrestricted host root, because the daemon runs as root and will create containers on your behalf. Agents often run in environments where this socket has been bound in for convenience, quietly handing over the whole machine.",
            "Code running as you can ask the Docker daemon to start a container that mounts the host root filesystem, then read or rewrite any root-owned file - escaping any sandbox and taking over the host without ever needing a password or an exploit.",
        ),
        "process.proc_environ" => (
            "On an un-sandboxed Linux box, one process can read the environment of every other process running as the same user, and environments routinely hold tokens, database passwords, and API keys. The agent shares your uid, so every secret another of your processes was started with is in reach.",
            "The agent can read /proc/<pid>/environ for your other running processes - your editor, a dev server, a background job - and harvest the API keys, database URLs, and tokens those processes were launched with, straight from kernel memory with no file on disk involved.",
        ),
        "process.cmdline_secrets" => (
            "Secrets passed as command-line arguments (--token=..., mysql -pPASS) are visible in /proc to every process running as the same user, so they leak well beyond the program that was invoked. The agent shares your uid and can read all of them.",
            "The agent can list the full command line of your other running processes and lift any password or token someone typed as a flag - logging into the same database, cloud account, or API the original command authenticated to.",
        ),
        "process.memory_introspection" => (
            "When kernel.yama.ptrace_scope is permissive, any process can attach to and dump the memory of another process owned by the same user - and that memory holds decrypted keys and live session secrets that never touch disk. The agent runs as you, so your most sensitive in-RAM material is exposed.",
            "The agent can attach to your ssh-agent or gpg-agent and read decrypted private keys, or dump your browser's memory for session cookies and saved passwords - capturing secrets that are deliberately never written to a file, directly from another process's RAM.",
        ),
        "process.sandbox_detect" => (
            "This tells you whether the agent's own process is actually contained, which is the lens for reading every other finding as live versus partially mitigated. If the process is not sandboxed, the reachable surfaces elsewhere are real exposures, not hypotheticals.",
            "When this reports the process is not contained (no isolating namespaces, no active seccomp filter, no proxy mediation, environ self-readable), it confirms the agent operates with your full ambient authority - so the other process, host, and credential findings apply at face value rather than being walled off by a sandbox.",
        ),
        "host.local_services" => (
            "Developers run datastores and admin panels bound to 127.0.0.1 with weak or no authentication, trusting that only they can reach localhost. A coding agent running as you reaches localhost too, so 'only I can connect' is no longer true.",
            "The agent can connect to your local Postgres, Redis, Mongo, or Elasticsearch and read or dump their full contents, or hit an unauthenticated local admin UI - reaching data and controls you assumed were private to your own keyboard.",
        ),
        "host.autostart_sinks" => (
            "Home-directory autostart locations (login shells, desktop autostart, user services) run automatically outside any sandbox the next time you log in or the session restarts. A file the agent can write there becomes code that executes later with your full identity.",
            "The agent can plant an entry in a writable home autostart sink that runs on your next login or shell start - establishing persistence that survives the session and executes unsandboxed as you, long after the agent that wrote it is gone.",
        ),
        "host.deferred_exec_sinks" => (
            "These repo-scoped files (package.json lifecycle scripts, Makefiles, direnv, editor/config hooks) execute later, outside the Bash sandbox, whenever you run a build, test, or open the workspace. The unsandboxed Write/Edit tools can plant code here that the sandbox never sees.",
            "The agent can write a build/test/lifecycle hook in your repo that runs with your full authority the moment you run the project or your editor loads it - turning an ordinary 'npm test' or directory open into arbitrary code execution outside any containment.",
        ),
        "host.writable_git_hooks" => (
            "Git hooks and git config in a repo execute commands automatically on routine git operations (commit, checkout, push), entirely outside any sandbox. If they are writable, the agent can arrange for its own code to run on your next ordinary git action.",
            "The agent can write a pre-commit, post-checkout, or other hook (or a config directive like core.hooksPath) so that your next commit or checkout silently runs its code as you - executing unsandboxed every time you use git in that repo.",
        ),
        "host.writable_persistence_paths" => (
            "Shell rc/profile files, high-value home dotfiles, and directories on your $PATH are the classic persistence surface: code placed there runs in future shells or shadows commands you type. Writability here is exactly what a sandbox is supposed to lock down.",
            "The agent can append to a writable .bashrc/.zshrc or profile so its code runs in every new shell, or drop a malicious binary into a writable $PATH directory that shadows a real command - giving it durable, automatic re-execution as you across future sessions.",
        ),
        "host.sandbox_binary_integrity" => (
            "The sandbox runtime execs its enforcement binaries (bwrap, socat) by bare name off $PATH with no integrity check, so whichever copy is first on $PATH at exec time effectively IS the sandbox. If the running user can write or shadow that resolution, the agent can subvert its own containment - the runc CVE-2019-5736 class of problem.",
            "Code running as you can place or replace a bwrap/socat earlier on $PATH (or overwrite the resolved binary) so the 'sandbox' the next agent run launches is actually attacker-controlled - neutralizing the very containment meant to restrict the agent before it even starts.",
        ),
        "host.network_config" => (
            "A writable /etc/hosts or /etc/resolv.conf lets code silently redirect domains and DNS, and NetworkManager connection profiles store WiFi and VPN secrets on disk. Either gives the agent a way to reroute traffic or harvest stored network credentials.",
            "If those resolver files are writable, the agent can repoint a package mirror, registry, or internal host to a server it controls and man-in-the-middle that traffic; if NetworkManager profiles are readable, it can lift the stored WiFi PSKs and VPN credentials.",
        ),
        "claude_code.writable_control_surface" => (
            "These are the files that constrain or instruct the agent - settings.json, CLAUDE.md, AGENTS.md, and the skills/commands/agents directories. If the agent can rewrite the rules meant to bound it, those rules stop being a control.",
            "Code running as the agent can edit a writable settings.json to disable its own sandbox, add an excludedCommands entry, or register a hook; or write a CLAUDE.md/AGENTS.md or skill that injects instructions into every future session - making a one-time foothold persistent and self-authorizing.",
        ),
        "claude_code.writable_control_surface.repo" => (
            "The same self-weakening and prompt-injection-persistence risk applies to the project-scoped control files inside the current repo (its .claude settings, CLAUDE.md/AGENTS.md, commands/skills). These ride along with the repo and load automatically when the project is opened.",
            "The agent can write project-local instruction or settings files so that its directives or loosened permissions activate every time anyone (including you) opens this repo with Claude Code - persisting the injection in the repo and spreading it to anyone who checks it out.",
        ),
        "claude_code.sandbox_posture" => (
            "This reports the declared Claude Code configuration - whether sandboxing is on, which escape hatches or weakenings are present, and how large the un-sandboxed tool surface (MCP servers, hooks) is. A weak declared posture means the agent is already operating with fewer guardrails than you may assume.",
            "When sandboxing is disabled or riddled with exceptions, or many MCP servers and hooks run un-sandboxed, the agent's actions are not contained - so the reachable credentials, processes, and host-write surfaces flagged elsewhere are exercisable directly, with no enforcement layer standing between the agent and your machine.",
        ),
        _ => return None,
    };
    Some(pair)
}

/// Per-`FindingClass` (why, how) fallback for finding ids without bespoke copy.
/// `class` is matched against `FindingClass::to_string()` values.
pub fn finding_impact_class(class: &str) -> (&'static str, &'static str) {
    match class {
        "Credentials" => (
            "This is a reachable credential store — material that authenticates as you. Anything running with your identity can read it without a further prompt.",
            "Code running as you can read these credentials and reuse them against whatever service they unlock — acting as you with no separate login — and, if egress is open, copy them off the machine.",
        ),
        "CrossRepo" => (
            "A coding agent's reach is your whole home, not just the repo it was launched in, so the source and secrets of every neighboring checkout are equally within reach.",
            "Code running as you can read the source, configs, and credentials of sibling repositories and use those secrets to reach the databases, cloud accounts, and APIs of unrelated projects.",
        ),
        "GitWrite" => (
            "Changes an agent makes here are not confined to local disk: working git credentials mean code can be published to the remote that teammates and deploy systems trust.",
            "An agent can commit and push to the remote, altering shared branches, CI/CD pipelines, and release artifacts; branch protection is enforced server-side and is not verified locally.",
        ),
        "Egress" => (
            "An open outbound path is the exit door for anything sensitive an agent gathers, and the channel for pulling in attacker-chosen instructions or payloads to run next.",
            "Code running as you can open outbound connections to exfiltrate the credentials, source, and host details found by the other probes, or to download additional tooling.",
        ),
        "Process" => (
            "On an un-sandboxed box, the agent shares your uid, so it can inspect every other process you are running — and their environments, command lines, and memory hold live secrets.",
            "The agent can read other processes' environment, command-line arguments, or memory to harvest tokens, database URLs, and decrypted keys that never touched disk, and a reachable daemon socket can hand over the host.",
        ),
        "HostPersistence" => (
            "These are the autostart, shell-rc, git-hook, and config surfaces that execute later, outside any sandbox; a file written here becomes code that runs with your full identity on a future login, build, or git action.",
            "The agent can plant a hook, rc-file line, or autostart entry that runs unsandboxed as you the next time you log in, build, or use git — establishing durable persistence that outlives the agent that wrote it.",
        ),
        "SystemInfo" => (
            "This reports sensitive host and system state — the surface that is reachable now and the blast radius that sits one escalation away — which frames how live the other findings are.",
            "The agent can read the system state inventoried here, and any target shown as already readable is reachable right now; once root is reached, every high-value system target on the box is exposed.",
        ),
        _ => (
            "This is a reachable surface within the agent's blast radius — something code running as you can touch without a further prompt.",
            "Code running as you can act on this surface with your full ambient authority, and if egress is open the results can leave the machine.",
        ),
    }
}

/// One-line "why" for each session signal id. `None` when the signal has no copy.
pub fn signal_impact(signal: &str) -> Option<&'static str> {
    let why = match signal {
        "read_secret" => "The agent read from a credential store (cloud profiles, SSH keys, the git credential store, a .env, or a browser session store) during this session — it touched material that authenticates as you.",
        "network_access" => "The agent made outbound network contact (an external fetch or connection), confirming this session can reach off-host destinations.",
        "shell_command" => "The agent ran a shell command, so it wasn't confined to reading and editing files — it executed arbitrary code with your shell's full reach.",
        "dangerous_shell_pattern" => "A command matched a high-danger shape (piping a download straight into a shell, recursive force-delete, world-writable chmod, or base64-decoding into execution) — patterns that fetch-and-run unseen code or destroy data.",
        "modified_production_deploy" => "The agent edited a production deploy artifact — a CI/CD workflow or Kubernetes/deploy manifest — files that define what runs in your live environment.",
        "edited_auth_payment_security_code" => "The agent modified code in an authentication, payment, or security-sensitive path — logic that gates access, moves money, or enforces protections.",
        "modified_dependency_manifest" => "The agent edited a dependency manifest or lockfile (package.json, Cargo.toml, requirements.txt, go.mod, or a lock), changing what third-party code your project trusts.",
        "external_mcp_call" => "The agent invoked a tool on a non-local MCP server, so part of this session's work and data flowed to an external service.",
        "human_approved_risky_action" => "A risky action in this session carried a human approval, meaning a person reviewed and consented to it rather than the agent acting unsupervised.",
        _ => return None,
    };
    Some(why)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_id_has_copy() {
        let (why, how) = finding_impact("aws.credentials.profiles").expect("known id");
        assert!(why.contains("AWS credentials file"));
        assert!(how.contains("AWS API"));
    }

    #[test]
    fn unknown_id_is_none() {
        assert!(finding_impact("totally.made.up").is_none());
    }

    #[test]
    fn class_fallback_covers_every_class() {
        for c in [
            "Credentials",
            "CrossRepo",
            "GitWrite",
            "Egress",
            "Process",
            "HostPersistence",
            "SystemInfo",
        ] {
            let (why, how) = finding_impact_class(c);
            assert!(!why.is_empty(), "{c} why");
            assert!(!how.is_empty(), "{c} how");
        }
    }

    #[test]
    fn signal_copy_present_and_missing() {
        assert!(signal_impact("read_secret").is_some());
        assert!(signal_impact("human_approved_risky_action").is_some());
        assert!(signal_impact("nope").is_none());
    }
}
