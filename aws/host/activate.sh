#!/usr/bin/env bash
# Runs from the downloaded release directory as root via SSM.
set -euo pipefail
mountpoint -q /srv/poketeam
exec 9>/srv/poketeam/deploy.lock
flock -n 9 || { echo 'Another deployment is running.' >&2; exit 1; }
release_dir=$PWD
source release.env
[[ $RELEASE =~ ^[0-9]{8}T[0-9]{6}Z-[a-f0-9]{8}$ ]]
[[ $PUBLIC_ORIGIN =~ ^https://[a-z0-9]+\.cloudfront\.net$ ]]
origin_token=$(cat /srv/poketeam/origin-token)
[[ $origin_token =~ ^[a-f0-9]{64}$ ]]
docker load -i image.tar.gz >/dev/null
cp host/compose.yaml compose.yaml
cp host/backup.sh backup.sh
chmod 700 backup.sh
sed "s/__ORIGIN_TOKEN__/$origin_token/" host/nginx.conf.template > nginx.conf
chmod 600 nginx.conf
install -d -m 755 /srv/poketeam/public/releases/placeholder
if [[ ! -e /srv/poketeam/public/current ]]; then
    printf '<!doctype html><title>Poketeam</title><p>The Poketeam API is running. The frontend has not been deployed yet.</p>\n' > /srv/poketeam/public/releases/placeholder/index.html
    ln -s releases/placeholder /srv/poketeam/public/current
fi
if [[ -f /srv/poketeam/runtime.env ]]; then
    cp /srv/poketeam/runtime.env /srv/poketeam/previous.env
    source /srv/poketeam/runtime.env
else
    POSTGRES_PASSWORD=$(openssl rand -hex 32)
fi
# Keep new release settings after loading persistent credentials from the old environment.
source release.env
cat > /srv/poketeam/runtime.env <<ENV
APP_IMAGE=poketeam:$RELEASE
DATABASE_URL=postgres://poketeam:$POSTGRES_PASSWORD@db:5432/poketeam
POSTGRES_PASSWORD=$POSTGRES_PASSWORD
PUBLIC_ORIGIN=$PUBLIC_ORIGIN
BUCKET=$BUCKET
ENV
chmod 600 /srv/poketeam/runtime.env
compose() { docker compose --env-file /srv/poketeam/runtime.env -f "$release_dir/compose.yaml" "$@"; }
compose up -d --wait db
# Back up before migrations on an existing deployment. Stop writes for a consistent upgrade boundary.
previous=
[[ ! -L /srv/poketeam/current ]] || previous=$(readlink -f /srv/poketeam/current)
if [[ -n $previous && -f $previous/compose.yaml ]]; then
    docker compose --env-file /srv/poketeam/previous.env -f "$previous/compose.yaml" stop web api worker
    bash "$previous/backup.sh"
fi
compose run --rm migrate
compose up -d api worker web
printf 'X-Poketeam-Origin: %s\n' "$origin_token" > proxy-health.headers
chmod 600 proxy-health.headers
for attempt in {1..60}; do
    if curl --fail --silent http://127.0.0.1:3000/health/live >/dev/null &&
        curl --fail --silent --header @proxy-health.headers http://127.0.0.1/health/live >/dev/null; then break; fi
    sleep 2
done
curl --fail --silent http://127.0.0.1:3000/health/live >/dev/null
curl --fail --silent --header @proxy-health.headers http://127.0.0.1/health/live >/dev/null
[[ $(curl --silent --output /dev/null --write-out '%{http_code}' http://127.0.0.1/health/live) == 403 ]]
rm proxy-health.headers
# Publish the configuration pointer only after the new API process is live.
[[ -z $previous ]] || ln -sfn "$previous" /srv/poketeam/previous
ln -sfn "$release_dir" /srv/poketeam/current
cat > /etc/systemd/system/poketeam-backup.service <<'UNIT'
[Unit]
Description=Poketeam PostgreSQL backup to private S3
Requires=docker.service
After=docker.service
RequiresMountsFor=/srv/poketeam
[Service]
Type=oneshot
ExecStart=/bin/bash /srv/poketeam/current/backup.sh
UNIT
cat > /etc/systemd/system/poketeam-backup.timer <<'UNIT'
[Unit]
Description=Daily Poketeam database backup
[Timer]
OnCalendar=*-*-* 04:00:00 UTC
Persistent=true
RandomizedDelaySec=900
[Install]
WantedBy=timers.target
UNIT
systemctl daemon-reload
systemctl enable --now poketeam-backup.timer
rm image.tar.gz release.tar.gz
# Keep current and previous application images; never prune other repositories or in-use images.
previous_image=
[[ ! -f /srv/poketeam/previous.env ]] || previous_image=$(sed -n 's/^APP_IMAGE=//p' /srv/poketeam/previous.env)
while IFS= read -r image; do
    [[ $image == "poketeam:$RELEASE" || $image == "$previous_image" || $image == 'poketeam:<none>' ]] && continue
    docker image rm "$image" >/dev/null || echo "Retained in-use image: $image"
done < <(docker image ls poketeam --format '{{.Repository}}:{{.Tag}}')
echo 'Backend deployed. The worker will initialize or refresh the catalog from its persisted schedule.'
