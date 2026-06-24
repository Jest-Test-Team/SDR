*** Settings ***
Documentation     End-to-end tests for the Secure Telemetry Gateway command surface
...               exposed by hil-simulator (CMD_NET_TOGGLE_DOWNSTREAM, simulated
...               SNMP set/get, system health, MAC deauth, node registration).
Library           OperatingSystem
Library           Process
Library           String
Suite Setup       Start Gateway Simulator
Suite Teardown    Terminate All Processes    kill=True

*** Variables ***
${ROOT}           ${CURDIR}/..
${PY}             python3
${CLIENT}         ${CURDIR}/software_sim_client.py
${PORT}           18095
${BASE}           http://127.0.0.1:${PORT}
${GW}             ${BASE}/api/v1/gateway
${GW_CMD}         ${BASE}/api/v1/gateway/command

*** Test Cases ***
Gateway Starts With Downstream Online In ApSta Mode
    ${out}=    Gateway Get
    Should Contain    ${out}    "wifi_mode": "ap_sta"
    Should Contain    ${out}    "downstream_online": true

Simulated SNMP Set Then Get Roundtrips
    Gateway Command    {"command":"snmp_set","oid":"1.3.6.1.4.1.custom.isolate","value":"true"}
    ${out}=    Gateway Command    {"command":"snmp_get","oid":"1.3.6.1.4.1.custom.isolate"}
    Should Contain    ${out}    "ok": true
    Should Contain    ${out}    "value": "true"

Toggle Downstream Switches To Sta Mode
    ${out}=    Gateway Command    {"command":"net_toggle_downstream"}
    Should Contain    ${out}    "wifi_mode": "sta"
    Should Contain    ${out}    "downstream_online": false
    # restore for later tests
    Gateway Command    {"command":"net_toggle_downstream"}

Snmp Get Fails When Downstream Offline
    Gateway Command    {"command":"net_toggle_downstream"}
    ${out}=    Gateway Command    {"command":"snmp_get","oid":"1.3.6.1.4.1.custom.relay"}
    Should Contain    ${out}    "ok": false
    Gateway Command    {"command":"net_toggle_downstream"}

Register And Deauth Station
    ${out}=    Gateway Command    {"command":"register_node","mac":"AA:BB:CC:DD:EE:01","ip":"192.168.4.9"}
    Should Contain    ${out}    "ok": true
    ${dup}=    Gateway Command    {"command":"register_node","mac":"aa:bb:cc:dd:ee:01","ip":"192.168.4.9"}
    Should Contain    ${dup}    "ok": false
    ${kick}=    Gateway Command    {"command":"deauth_sta","mac":"AA:BB:CC:DD:EE:01"}
    Should Contain    ${kick}    "ok": true

Sys Health Reports Free Heap
    ${out}=    Gateway Command    {"command":"sys_health"}
    Should Contain    ${out}    "ok": true
    Should Contain    ${out}    "free_heap_bytes"

*** Keywords ***
Start Gateway Simulator
    ${tmp}=    Evaluate    tempfile.mkdtemp(prefix="sdr-gw-")    modules=tempfile
    Start Process    cargo    run    --quiet    -p    hil-simulator    --    --port    ${PORT}
    ...    cwd=${ROOT}    stdout=${tmp}/hil.log    stderr=STDOUT    alias=gateway-sim
    Run Process    ${PY}    ${CLIENT}    wait-url    ${BASE}/api/v1/status    --timeout    60

Gateway Get
    ${result}=    Run Process    ${PY}    ${CLIENT}    get-json    ${GW}
    Should Be Equal As Integers    ${result.rc}    0
    RETURN    ${result.stdout}

Gateway Command
    [Arguments]    ${body}
    ${result}=    Run Process    ${PY}    ${CLIENT}    post-json    ${GW_CMD}    ${body}
    Should Be Equal As Integers    ${result.rc}    0
    RETURN    ${result.stdout}
