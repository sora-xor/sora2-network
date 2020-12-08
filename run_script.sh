#!/bin/sh

export RUST_LOG="sc_rpc=trace"

function local_id() {
  awk "
    BEGIN { a=1 }
    /Local node identity is: /{
      if (a) {
        print \$10 > \"/tmp/localid.txt\";
        fflush();
        a=0
      }
    }
    { print \"LOG: \" \$0; fflush() }
  "
}

port="10000"
start1="1"
for name in alice bob charlie dave eve
do
	newport=`expr $port + 1`
	if [ "$start1" == "1" ]; then
		sh -c "./target/release/framenode --tmp --$name --port $newport --chain local 2>&1" | local_id > /tmp/port_${newport}_name_$name.txt &
	else
		sh -c "./target/release/framenode --tmp --$name --port $newport --chain local --bootnodes /ip4/127.0.0.1/tcp/$port/p2p/`cat /tmp/localid.txt` 2>&1" | local_id > /tmp/port_${newport}_name_$name.txt &
	fi
	echo $newport $port $name
	sleep 5
	port="$newport"
	start1="0"
done

echo test

#sleep 900

#pkill -KILL framenode

wait
