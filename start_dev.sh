children=()

_term() {
    echo "Caught SIGTERM"
    for child in "${children[@]}"; do
        kill -TERM "$child" 2>/dev/null
    done 
}

_int() {
    echo "Caught SIGINT"
    for child in "${children[@]}"; do
        kill -TERM "$child" 2>/dev/null
    done 
}

trap _term SIGTERM
trap _int SIGINT

pushd src;

pushd server;
cargo watch -x "run" &
SERVER_PROC=$!
children+=($SERVER_PROC)
popd;

pushd web;
trunk serve &
WEB_PROC=$!
children+=($WEB_PROC)
popd;

wait $SERVER_PROC