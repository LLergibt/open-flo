#!/bin/sh
set -eu

conf=/etc/nginx/conf.d/default.conf
stage=$(mktemp /etc/nginx/conf.d/.default.XXXXXX)
trap 'rm -f "$stage"' EXIT
if [ -s /tls/current/fullchain.pem ] && [ -s /tls/current/privkey.pem ]; then
    sed "s|@HTTP_ACTION@|return 308 ${PUBLIC_ORIGIN}\$request_uri;|" /deploy/http.conf.template > "$stage"
    cat /deploy/https.conf >> "$stage"
else
    sed 's|@HTTP_ACTION@|return 404;|' /deploy/http.conf.template > "$stage"
fi
mv -f "$stage" "$conf"
nginx -t
if [ "${1:-}" = start ]; then
    exec nginx -g 'daemon off;'
fi
nginx -s reload
