# Embedded after the DATA_VOLUME_ID assignment in EC2 user data. Runs as root.
exec > >(tee -a /var/log/poketeam-bootstrap.log) 2>&1
dnf install -y docker awscli-2 jq xfsprogs
install -d -m 755 /usr/local/lib/docker/cli-plugins
compose_asset="docker-compose-linux-$(uname -m)"
compose_url=https://github.com/docker/compose/releases/download/v5.6.0
cd /usr/local/lib/docker/cli-plugins
curl --fail --location --retry 3 "$compose_url/$compose_asset" -o "$compose_asset"
curl --fail --location --retry 3 "$compose_url/checksums.txt" -o checksums.txt
grep -E " [*]?$compose_asset\$" checksums.txt | sha256sum --check --strict
mv "$compose_asset" docker-compose
chmod 755 docker-compose
rm checksums.txt

# A separately managed encrypted data disk survives deletion/replacement of the host.
serial=${DATA_VOLUME_ID//-/}
device=
for attempt in {1..120}; do
    device=$(lsblk -dn -o PATH,SERIAL | awk -v serial="$serial" '$2 == serial {print $1}')
    [[ -b $device ]] && break
    sleep 2
done
[[ -b $device ]] || { echo 'Data volume did not attach.' >&2; exit 1; }
filesystem=$(blkid -s TYPE -o value "$device" || true)
if [[ -z $filesystem ]]; then
    # Refuse to format a disk with partitions or unidentified signatures.
    [[ $(lsblk -nr -o NAME "$device" | wc -l) == 1 ]]
    [[ -z $(wipefs --no-act --noheadings "$device") ]]
    mkfs.xfs "$device"
elif [[ $filesystem != xfs ]]; then
    echo "Refusing to format existing $filesystem data volume." >&2; exit 1
fi
install -d -m 700 /srv/poketeam
volume_uuid=$(blkid -s UUID -o value "$device")
grep -q "UUID=$volume_uuid " /etc/fstab || printf 'UUID=%s /srv/poketeam xfs defaults,nofail,x-systemd.device-timeout=120 0 2\n' "$volume_uuid" >> /etc/fstab
mountpoint -q /srv/poketeam || mount /srv/poketeam
mountpoint -q /srv/poketeam
chmod 700 /srv/poketeam

# Containers must never start on an empty root-disk mount directory after a reboot.
install -d /etc/systemd/system/docker.service.d
cat > /etc/systemd/system/docker.service.d/poketeam-data.conf <<'UNIT'
[Unit]
RequiresMountsFor=/srv/poketeam
After=srv-poketeam.mount
UNIT
if [[ ! -f /swapfile ]]; then
    dd if=/dev/zero of=/swapfile bs=1M count=2048 status=none
    chmod 600 /swapfile
    mkswap /swapfile
fi
swapon /swapfile || [[ $(swapon --noheadings --show=NAME) == *'/swapfile'* ]]
grep -q '^/swapfile ' /etc/fstab || echo '/swapfile none swap sw 0 0' >> /etc/fstab
systemctl daemon-reload
systemctl enable --now docker amazon-ssm-agent

# Also records successful explicit recovery if cloud-init retains an earlier failure.
install -d -m 700 /var/lib/poketeam
touch /var/lib/poketeam/bootstrap-complete
