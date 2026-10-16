#!/bin/sh
# Run as a user with sudo, from this directory: builds the image as ghrunner and installs the service.
set -eu
sudo install -d -o root -g root -m 755 /opt/ghrunner
sudo install -m 644 supervisor.py /opt/ghrunner/supervisor.py
sudo install -m 644 Containerfile /opt/ghrunner/Containerfile
sudo install -m 644 ../../../rust-toolchain.toml /opt/ghrunner/rust-toolchain.toml
# setarch -R (valgrind, the callgrind counter) needs personality(ADDR_NO_RANDOMIZE), which the default allows only for a few values.
# The container default profile with perf_event_open allowed outright (its default rules deny it without CAP_SYS_ADMIN): the instruction counters (measure, the scaling tests).
jq '.syscalls |= (map(.names -= ["perf_event_open","personality"] | select(.names|length>0)) + [{names:["perf_event_open","personality"],action:"SCMP_ACT_ALLOW"}])' /usr/share/containers/seccomp.json | sudo tee /opt/ghrunner/seccomp.json >/dev/null
sudo -u ghrunner -H sh -c 'cd /opt/ghrunner && podman build -q -t llrm-runner -f Containerfile .'
sudo systemctl disable --now ghrunner.service 2>/dev/null || true
sudo rm -f /etc/systemd/system/ghrunner.service
sudo install -m 644 ghrunner@.service ghrunner-heartbeat.service /etc/systemd/system/
n=0
for cpus in $(python3 supervisor.py cpusets); do
  n=$((n + 1))
  sudo install -d /etc/systemd/system/ghrunner@$n.service.d
  printf '[Service]\nAllowedCPUs=%s\n' "$cpus" | sudo tee /etc/systemd/system/ghrunner@$n.service.d/cpus.conf >/dev/null
done
sudo systemctl daemon-reload
for i in $(seq 1 $n); do sudo systemctl enable ghrunner@$i.service >/dev/null 2>&1; sudo systemctl restart ghrunner@$i.service; done
sudo systemctl enable ghrunner-heartbeat.service >/dev/null 2>&1
sudo systemctl restart ghrunner-heartbeat.service
