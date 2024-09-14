document.addEventListener('DOMContentLoaded', () => {
    window.resolver = {
        sort : "network",
        nodes : [],
        networks : {
            "mainnet": true,
            "testnet-10": true,
            "testnet-11": true,
        },
    };

    table = document.createElement('table');
    document.body.appendChild(table);

    thead = document.createElement('thead');
    table.appendChild(thead);
    thead.innerHTML = "<tr><th>SID:UID</th><th>SERVICE</th><th>VERSION</th><th>NETWORK</th><th>STATUS</th><th class='right'>PEERS</th><th class='right'>CLIENTS / CAP</th><th class='right'>LOAD</th></tr>";

    tbody = document.createElement('tbody');
    tbody.id = "nodes";
    table.appendChild(tbody);

    fetchData();
});

function pad(str, len) {
    return str.length >= len ? str : ' '.repeat(len - str.length) + str;
}

function fetchData() {
    fetch('/json')
        .then(response => response.json())
        .then(data => {
            window.resolver.nodes = data;
            render();
            setTimeout(fetchData, 7500);
        })
        .catch(error => {
            setTimeout(fetchData, 1000);
            console.error('Error fetching data:', error);
        });
}

function filter(node, ctx) {
    if (!window.resolver.networks[node.network]) {
        return "hidden";
    } else if (node.status == "offline" && !ctx.offline) {
        return "hidden";
    } else if (node.status == "delegator" && !ctx.delegators) {
        return "hidden";
    } else {
        return node.status;
    }
}

function render() {

    let ctx = {
        offline : true,
        delegators : false,
    };

    let resort = false;
    let tbody = document.getElementById("nodes");

    const status = window.resolver.nodes
        .filter((node) => node.status === 'online')
        .reduce((acc, node) => {
            const network = node.network;
            if (!acc[network]) {
                acc[network] = { clients: 0, capacity: 0, count : 0 };
            }
            acc[network].clients += node.clients;
            acc[network].capacity += node.capacity;
            acc[network].count += 1;
            return acc;
        }, {});

    if (window.resolver.sort != window.resolver.lastSort) {
        resort = true;
        window.resolver.lastSort = window.resolver.sort;
    }

    window.resolver.nodes.forEach((node) => {
        let { 
            version,
            sid,
            uid,
            service,
            url,
            protocol,
            encoding,
            encryption,
            network,
            cores,
            memory,
            status,
            peers,
            clients,
            capacity,
            delegates,
        } = node;

        let el = document.getElementById(uid);
        if (!el) {
            el = document.createElement('tr');
            el.id = uid;
            el.setAttribute('data-sort', node.network);
            tbody.appendChild(el);
            resort = true;
        }
        el.className = filter(node, ctx);

        let load = (clients / capacity * 100.0).toFixed(2);
        let peers_ = pad(peers.toLocaleString(),4);
        let clients_ = pad(clients.toLocaleString(),6);
        let capacity_ = pad(capacity.toLocaleString(),6);
        el.innerHTML = `<td>${sid}:${uid}</td><td>${service}</td><td>${version}</td><td>${network}</td><td>${status}</td>`;
        if (status != "offline") {
            el.innerHTML += `<td class='wide right pre'>${peers_}</td><td class='wide right pre'>${clients_} / ${capacity_}</td><td class='wide right'>${load}%</td>`;
        }
    });

    if (resort) {
        sort();
    }

    let status_entries = Object.entries(status);
    status_entries.sort(([a], [b]) => a.localeCompare(b));
    status_entries.forEach(([network, status]) => {
        let el = document.getElementById(`${network}-data`);
        if (!el) {
            let tbody = document.getElementById("status");
            el = document.createElement('td');
            el.id = network;
            el.innerHTML = `<input type="checkbox" id="${network}-filter" checked> <label for="${network}-filter">${network}: <span id='${network}-data'></span></label>&nbsp;&nbsp;&nbsp;`;
            tbody.appendChild(el);

            if (window.resolver.networks[network] == undefined) {
                window.resolver.networks[network] = true;
            }

            document.getElementById(`${network}-filter`).addEventListener('change', () => {
                window.resolver.networks[network] = document.getElementById(`${network}-filter`).checked;
                render();
            });
            el = document.getElementById(`${network}-data`);
        }
        let load = (status.clients / status.capacity * 100.0).toFixed(2);
        let count = status.count.toLocaleString();
        let clients = status.clients.toLocaleString();
        let capacity = status.capacity.toLocaleString();
        el.innerHTML = `(${count}) ${clients} / ${capacity} &nbsp;${load}%`;
    });

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
