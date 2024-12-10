# NGINX Healthchecker for Nolus Protocol
Welcome to the NGINX Healthchecker repository for the Nolus Protocol.  
This project provides a simple and effective way to monitor the health of services behind an NGINX proxy, ensuring high availability and proper failover handling.

## Features
* Health Checks: Periodically pings specified services to ensure they are up and running.
* Dynamic Updates: Automatically updates NGINX configuration based on the health of services.
* Customizable: Supports a variety of health check parameters for flexibility.
* Lightweight: Minimal resource usage, ensuring optimal performance.

## Prerequisites
* Rust compiler, version 1.79 or later. Bundled in Rustup-provided toolchains.
* Cargo build system. Bundled in Rustup-provided toolchains.
* C compiler.

**Important:** The service expects NGINX to be ran as a `systemd` service unit, as it invokes `systemctl reload` to notify NGINX of changes to it's configuration file.

## Building
1. Clone the repository
   ```shell
   git clone "https://github.com/nolus-protocol/nginx-healthchecker.git"

   cd "./nginx-healthchecker/"
   ```
2. Build
  * Portable.
    ```shell
    cargo build --jobs "1" --release
    ```
  * Non-portable, allowing more optimizations and targetting host CPU features.
    ```shell
    RUSTFLAGS="-C target-cpu=native" \
      cargo build --jobs "1" --release
    ```

The built executable is located at: `./target/release/nginx-healthchecker`.

## Installation via Cargo
* Portable.
  ```shell
  cargo \
    install \
    --jobs "1" \
    --git "https://github.com/nolus-protocol/nginx-healthchecker.git"
  ```
* Non-portable, allowing more optimizations and targetting host CPU features.
  ```shell
  RUSTFLAGS="-C target-cpu=native" \
    cargo \
      install \
      --jobs "1" \
      --git "https://github.com/nolus-protocol/nginx-healthchecker.git"
  ```

## Installation via GNU `install`
0. \(Steps from building section.\)
1. Install executable in a suitable path (used path: `/usr/local/bin/`)
   ```shell
   install \
     "./target/release/nginx-healthchecker" \
     "/usr/local/bin/"
   ```

# Configuration
The service uses two pieces of configuration, a static configuration, describing everything NGINX needs to do, and a dynamic one, describing the upstreams and the way to run healthchecks on its endpoints.

**Note:** Both configurations are loaded once and live through the lifetime of the process akin to NGINX, thus would not be affected by any changes on the files until restarted or reloaded. For information on how to reload the service, refer to the "Running" section.

## Static configuration
The static configuration is the original NGINX configuration that is used, *without* the addition of the upstreams as those are part of the dynamic configuration.  
The static configuration is written out as-is and then the upstreams definitions for each service are appended at the end.

**Note:** It is important to note that when the service is identified as a Tendermint-compatible node, produced upstream definitions inherit the name of the service itself, *while also* each appending `_lcd`, `_rpc` or `_grpc`, respective of it's upstream URLs/URIs. E.g.: for a `node` service named `node_service` the produced upstream sections will be:
* `node_service_lcd`,
* `node_service_rpc`,
* `node_service_grpc`.

### Example configuration
```nginx
server {
  listen 443 ssl http2;

  server_name example.com;

  ssl_certificate /path/to/certificate/certificate.pem;

  ssl_certificate_key /path/to/certificate/private_key.pem;

  location / {
    proxy_pass http://example_dot_com_grpc;

    proxy_http_version 1.1;
  }
}
```

## Dynamic configuration
The dynamic configuration represents a JavaScript Object Notation (JSON) file that stores information about upstreams, refresh period and other configurations vital to the service.  
Upstreams are grouped, like in the original NGINX configuration, and in the service those groups are be called "services" and as such will be referred to as such from here on.

**Note:** This is a high overview of the dynamic configuration.  
For in-depth descriptions, head over to [CONFIGURATION.md](CONFIGURATION.md).

The structure of the configuration is roughly equivalent to the following:
```json
{
  "refresh_seconds": 15,
  "prepend": "least_conn",
  "services": {
    "example_dot_com": {
      "type": "generic_200_ok",
      "prepend": "least_conn",
      "instances": {
        "example-upstream-1": {
          "healthcheck_url": "http://127.0.0.1:8080/healthcheck",
          "output": "server 127.0.0.1:8080"
        },
        "example-upstream-2": {
          "healthcheck_url": "http://127.0.0.2:8080/healthcheck",
          "output": "server 127.0.0.2:8080"
        }
      }
    }
  }
}
```

## Running
The service supports reloading it's configuration on-the-fly, allowing more flexibility without the need of a restart.  
The reloading of the configuration happens via sending the standard UNIX `SIGHUP` signal to the process.

### Running as a `systemd` service unit
The service was made with `systemd` in mind, so it can easily be ran as a `systemd` service unit.

### Example `systemd` service unit configuration
```ini
[Unit]
Description=NGINX Healthcheck Service
After=nginx.service

[Service]
Type=simple
ExecStart=/usr/local/bin/nginx-healthcheck # Replace with actual path
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
```

## Logging
When not in verbose mode, on each cycle the service logs the all the services' upstreams that failed the healthcheck.  
The upstreams, which changed state, from failing to succeeding, are logged once on the cycle when they changed state, indicating them as such.

When in verbose mode, on each cycle the service logs all the services' upstreams with their state, no matter whether failing or not, while indicating their respective state.

## Contributions
Contributions are welcome!  
Feel free to submit issues or pull requests to improve the healthchecker.

### License
This project is licensed under the [Apache-2.0 License](https://opensource.org/license/apache-2-0) \(SPDX short identifier: `Apache-2.0`\).
