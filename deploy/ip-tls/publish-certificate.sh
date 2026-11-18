#!/bin/sh
set -eu

source=/etc/letsencrypt/live/flo-ip
umask 077
generation=$(mktemp -d /tls/cert.XXXXXX)
cp "$source/fullchain.pem" "$generation/fullchain.pem"
cp "$source/privkey.pem" "$generation/privkey.pem"
ln -s "${generation##*/}" "$generation.link"
mv -Tf "$generation.link" /tls/current
for previous in /tls/cert.*; do
    [ "$previous" = "$generation" ] || rm -rf -- "$previous"
done
