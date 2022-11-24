# support_container

Reproduce potential deadlock scenario of ws receiver

# Before Flashing
Edit main.rs CONFIG with wifi_ssid + wifi_psk

Edit IP address of ws connection in oython scripts double_send_deadlock.py & double_send_working.py to ip of esp

# To Test:

python3 double_send_working.py
python3 doube_send_deadlock.oy
