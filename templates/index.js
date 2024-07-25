// // console.log("status.js loaded");

// function randomId() {
//     return (Math.round(Math.random()*1e8)).toString(16);
// }

// // Log to an element by its id
// function logToId(id, ...args) {
//     let el = document.getElementById(id);
//     if (!el) {
//         el = document.createElement('code');
//         el.id = id;
//         document.body.appendChild(el);
//     }

//     el.innerHTML = args.map((arg) => {
//         return typeof arg === 'object' ? stringify(arg) : arg;
//     }).join(' ') + "<br>";
// }

// // Clear the content of an element by its id
// function clearId(id) {
//     if (id) {
//         let el = document.getElementById(id);
//         if (el) {
//             el.innerHTML = '';
//         }
//     }
// }

// function log(...args) {
//     let el = document.createElement('code');
//     el.innerHTML = args.map((arg) => {
//         return typeof arg === 'object' ? stringify(arg) : arg;
//     }).join(' ') + "<br>";
//     document.body.appendChild(el);
// }


// function stringify(json) {
//     if (typeof json != 'string') {
//         json = JSON.stringify(json, (k, v) => { return typeof v === "bigint" ? v.toString() + 'n' : v; }, 2);
//     }
//     json = json.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"(\d+)n"/g,"$1n");
//     return json.replace(/("(\\u[a-zA-Z0-9]{4}|\\[^u]|[^\\"])*"(\s*:)?|\b(true|false|null)\b|-?\d+(?:\.\d*)?(?:[eE][+\-]?\d+)?n?)/g, function (match) {
//         var cls = 'number';
//         if (/^"/.test(match)) {
//             if (/:$/.test(match)) {
//                 cls = 'key';
//             } else {
//                 cls = 'string';
//             }
//         } else if (/true|false/.test(match)) {
//             cls = 'boolean';
//         } else if (/null/.test(match)) {
//             cls = 'null';
//         }
//         return '<span class="' + cls + '">' + match + '</span>';
//     });
// }


document.addEventListener('DOMContentLoaded', () => {
    table = document.createElement('table');
    document.body.appendChild(table);

    thead = document.createElement('thead');
    table.appendChild(thead);
    thead.innerHTML = "<tr><th>SID:UID</th><th>SERVICE</th><th>VERSION</th><th>FQDN</th><th>PROTO</th><th>ENCODING</th><th>NETWORK</th><th>STATUS</th><th class='right'>CONN/CAP</th><th class='right'>LOAD</th></tr>";

    tbody = document.createElement('tbody');
    tbody.id = "nodes";
    table.appendChild(tbody);

    ["offline", "delegators"].forEach((id) => {
        document.getElementById(id).addEventListener('change', () => {
            render();
        });
    });

    fetchData();
});

function fetchData() {
    fetch('/status/json')
        .then(response => response.json())
        .then(data => {
            setTimeout(fetchData, 5000);
            window.nodes = data;
            render();
        })
        .catch(error => {
            setTimeout(fetchData, 5000);
            console.error('Error fetching data:', error);
        });
}

function filter(node, ctx) {
    if (node.status == "offline" && !ctx.offline) {
        return "hidden";
    } else if (node.status == "delegator" && !ctx.delegators) {
        return "hidden";
    } else {
        return node.status;
    }
}

function render() {
    
    let ctx = {
        offline : document.getElementById('offline').checked,
        delegators : document.getElementById('delegators').checked,
    };

    let resort = false;
    let tbody = document.getElementById("nodes");

    window.nodes.forEach((node) => {
        let { 
            version,
            sid,
            uid,
            fqdn,
            service,
            url,
            protocol,
            encoding,
            encryption,
            network,
            cores,
            status,
            connections,
            capacity,
            delegates,
        } = node;

        let el = document.getElementById(uid);
        if (!el) {
            el = document.createElement('tr');
            el.id = uid;
            el.setAttribute('data-sort', fqdn);
            tbody.appendChild(el);
            resort = true;
        }
        el.className = filter(node, ctx);

        let load = (connections / capacity * 100.0).toFixed(2);
        el.innerHTML = `<td>${sid}:${uid}</td><td>${service}</td><td>${version}</td><td>${fqdn}</td><td>${protocol}</td><td>${encoding}</td><td>${network}</td><td>${status}</td>`;
        if (status != "offline") {
            el.innerHTML += `<td class='wide right'>${connections}/${capacity}</td><td class='wide right'>${load}%</td>`;
        }
    });

    if (resort) {
        sort();
    }
}

function sort() {
    let tbody = document.getElementById("nodes");
    let rows = Array.from(tbody.getElementsByTagName('tr'));
    rows.sort((a, b) => {
        let aValue = a.getAttribute('data-sort');
        let bValue = b.getAttribute('data-sort');
        return aValue.localeCompare(bValue);
    });
    tbody.innerHTML = ''; // Clear existing rows
    rows.forEach(row => tbody.appendChild(row));
}

/*
{
    "version": "v0.14.1-2eb1dfd3",
    "sid": "070904073564caa7",
    "uid": "ec5c21895e451a39",
    "url": "wss://ruby.kaspa.stream/kaspa/mainnet/wrpc/borsh",
    "protocol": "wrpc",
    "encoding": "borsh",
    "encryption": "tls",
    "network": "mainnet",
    "cores": 18,
    "status": "delegator",
    "connections": 145,
    "capacity": 18432,
    "delegates": ["[070904073564caa7] wss://lily.kaspa.stream/kaspa/mainnet/wrpc/borsh"]
}
*/