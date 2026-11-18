#!/usr/bin/env bash
set -euo pipefail
umask 077

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$root"
config="$root/.env.ip-tls"
# The checked-out deployment config is the single source of the public origin.
unset PUBLIC_ORIGIN

usage() {
    cat >&2 <<'HELP'
Usage:
  ./scripts/ip-tls.sh init PUBLIC_IP EMAIL
  ./scripts/ip-tls.sh renew
  ./scripts/ip-tls.sh dry-run
  ./scripts/ip-tls.sh install-timer
HELP
    exit 2
}

if docker compose version >/dev/null 2>&1; then
    compose=(docker compose)
elif command -v docker-compose >/dev/null; then
    compose=(docker-compose)
else
    echo "Docker Compose v2 or standalone Docker Compose 1.29.2 is required." >&2
    exit 1
fi
dc() {
    "${compose[@]}" -p open-source-flo --env-file "$config" -f "$root/compose.prod.yaml" "$@"
}
read_config() {
    if [ ! -s "$config" ]; then
        echo "Run init with your public IP address and email first." >&2
        exit 1
    fi
}
# A Linux server serializes issuance, renewal and the Nginx reload.
if command -v flock >/dev/null; then
    mkdir -p .data
    exec 9>.data/ip-tls.lock
    flock 9
fi

publish_certificate() {
    # Only the serving certificate and key are copied to the Nginx volume.
    dc run -T --rm --no-deps --entrypoint sh certbot /deploy/publish-certificate.sh
    dc exec -T web /bin/sh /deploy/configure-nginx.sh reload
}

case "${1:-}" in
    init)
        [ "$#" -eq 3 ] || usage
        ip="$2"
        email="$3"
        # Accept a literal public unicast IPv4 or IPv6 address, with no port or zone.
        origin="$(python3 - "$ip" "$email" <<'PY'
import ipaddress
import re
import sys

try:
    address = ipaddress.ip_address(sys.argv[1])
    if "%" in sys.argv[1] or not address.is_global or address.is_multicast or address.is_reserved:
        raise ValueError
except ValueError:
    sys.exit("Use a publicly routable unicast IPv4 or IPv6 address.")
if not re.fullmatch(r"[A-Za-z0-9_.+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}", sys.argv[2]):
    sys.exit("Enter an ACME contact email.")
host = f"[{address}]" if address.version == 6 else str(address)
print(f"https://{host}")
PY
)"
        if [ -e "$config" ]; then
            if ! grep -Fxq "PUBLIC_ORIGIN=$origin" "$config"; then
                echo "This checkout is configured for a different IP. See docs/DEPLOYMENT.md." >&2
                exit 1
            fi
        else
            printf 'PUBLIC_ORIGIN=%s\n' "$origin" > "$config"
        fi
        dc up -d --build api web
        dc run -T --rm --no-deps certbot certonly --non-interactive --agree-tos --email "$email" \
            --webroot --webroot-path /var/www/acme --ip-address "$ip" \
            --preferred-profile shortlived --cert-name flo-ip --keep-until-expiring
        publish_certificate
        printf 'HTTPS configured at %s\n' "$origin"
        ;;
    renew)
        [ "$#" -eq 1 ] || usage
        read_config
        dc up -d --no-build api web
        dc run -T --rm --no-deps certbot renew --non-interactive --cert-name flo-ip
        publish_certificate
        ;;
    dry-run)
        [ "$#" -eq 1 ] || usage
        read_config
        dc run -T --rm --no-deps certbot renew --dry-run --non-interactive --cert-name flo-ip
        ;;
    install-timer)
        [ "$#" -eq 1 ] || usage
        read_config
        if [ "$(id -u)" -ne 0 ] || ! command -v systemctl >/dev/null; then
            echo "Run install-timer as root on a Linux server with systemd." >&2
            exit 1
        fi
        # Keep the unit's ExecStart path unambiguous for systemd.
        case "$root" in *[!a-zA-Z0-9_./-]*) echo "Use a checkout path without spaces or special characters." >&2; exit 1 ;; esac
        sed "s|@PROJECT_ROOT@|$root|g" deploy/ip-tls/open-flo-ip-tls.service > /etc/systemd/system/open-flo-ip-tls.service
        install -m 0644 deploy/ip-tls/open-flo-ip-tls.timer /etc/systemd/system/open-flo-ip-tls.timer
        chmod 0644 /etc/systemd/system/open-flo-ip-tls.service
        systemctl daemon-reload
        systemctl enable --now open-flo-ip-tls.timer
        ;;
    *) usage ;;
esac
