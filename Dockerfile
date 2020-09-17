FROM lnl7/nix:2020-06-07

ENV GIT_SSL_CAINFO="/etc/ssl/certs/ca-certificates.crt" \
    SSL_CERT_FILE="/etc/ssl/certs/ca-certificates.crt" \
    INSIDE_DOCKER=1 \
    DONE_DIR="/usr/.docker_build_jobs"

ADD . /repos/parachain

RUN bash -c " \
nix-env -i which gawk gnused gnugrep gnutar || exit 1 ;\
. /repos/parachain/scripts/partial/helpers.sh || exit 1 ;\
skip_if_done '[0-9]' ;\
info 'copying needed certificate files' ;\
must mkdir -p /etc/ssl/certs ;\
verbose must cp /repos/parachain/misc/ca-certificates.crt /etc/ssl/certs/ ;\
set_done 0 \
"

RUN bash -c " \
. /repos/parachain/scripts/partial/helpers.sh || exit 1 ;\
skip_if_done '[1-9]' ;\
info 'creating build environment for docker' ;\
must mkdir /parachain ;\
must cd /parachain ;\
verbose bash /repos/parachain/scripts/inside_docker.sh ;\
set_done 1 \
"

RUN bash -c " \
. /repos/parachain/scripts/partial/helpers.sh || exit 1 ;\
skip_if_done '[2-9]' ;\
info 'cloning and setup parachain repository for specific commit' ;\
must cd /parachain ;\
must git clone /repos/parachain ;\
must mv parachain/* . ;\
must mv parachain/.[a-hj-z]* . ;\
verbose must git checkout 'docker/build' ;\
must mkdir -p /cache/target_current ;\
must ln -s /cache/target_current ./target ;\
set_done 2 \
"

RUN bash -c " \
. /repos/parachain/scripts/partial/helpers.sh || exit 1 ;\
skip_if_done '[3-9]' ;\
info 'nix-shell preinstall' ;\
must cd /parachain ;\
must nix-shell --run 'true' ;\
set_done 3 \
"

RUN bash -c " \
. /repos/parachain/scripts/partial/helpers.sh || exit 1 ;\
skip_if_done '[4-9]' ;\
info 'build polkadot' ;\
must cd /parachain ;\
must bash ./scripts/localtestnet.sh --just-compile-deps ;\
set_done 4 \
"

RUN bash -c " \
. /repos/parachain/scripts/partial/helpers.sh || exit 1 ;\
skip_if_done '[5-9]' ;\
info 'building and testing local test net' ;\
must cd /parachain ;\
must bash ./scripts/localtestnet.sh -e ;\
set_done 5 \
"

RUN bash -c " \
. /repos/parachain/scripts/partial/helpers.sh || exit 1 ;\
skip_if_done '[6-9]' ;\
info 'final step, move and clean files and dirs' ;\
verbose must mkdir -p /usr/local/bin ;\
verbose must mv /cache/polkadot_ready/target/release/polkadot /usr/local/bin ;\
verbose must rm -Rf /cache/polkadot_ready ;\
verbose must rm -Ff /parachain ;\
verbose must rm -Ff /repos ;\
verbose must mv /cache /cache_inside_image ;\
set_done 6 \
"

ENV CARGO_TARGET_DIR="/cache_inside_image/target_current"
