#!/usr/bin/env bash

cd "$(dirname "$0")" || exit

if [[ $2 == "l" ]]; then
  echo "Installing [l]oad balancer"
  ansible-playbook setup_lb.yml -i "hosts/$1.yml"
elif [[ $2 == "b" ]]; then
  echo "Deploying binary"
  cd ..
  docker run --rm -it -v "$(pwd)":/home/rust/src -v "$(pwd)/build_cache/git":/home/rust/.cargo/git -v "$(pwd)/build_cache/registry":/home/rust/.cargo/registry -v "$(pwd)/backend/target":/home/rust/src/target -e RUSTFLAGS="-C opt-level=3 -C debuginfo=0 -C target-cpu=znver1" ekidd/rust-musl-builder ./scripts/docker_build.sh
  cd scripts
  ansible-playbook copy_backend.yml -i "hosts/$1.yml"
elif [ "$2" == "s" ]; then
  echo "Preparing service"
  ansible-playbook setup_domain.yml -i "hosts/$1.yml"
  ansible-playbook setup_scheduler.yml -i "hosts/$1.yml"
elif [ "$2" == "i" ]; then
  cat "hosts/$1.yml"
else
  echo "Usage: deploy.sh <env> <command>"
  echo "    command: [f]rontend, [s]cheduler, [b]inary, [i] print config or [l]oad balancer"
fi
