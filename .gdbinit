while $core != 0 && $core != 1
  printf "Select core (0 for Core 0, 1 for Core 1): "
  shell read core_input
  set $core = (int)core_input
end

if $core == 0
  set $PORT = 3333
  echo "Connecting to Core 0 on port 3333\n"
else
  set $PORT = 3334
  echo "Connecting to Core 1 on port 3334\n"
end

target extended-remote localhost:$PORT
monitor reset init
load
monitor reset halt
