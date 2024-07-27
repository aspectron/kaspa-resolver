document.addEventListener('DOMContentLoaded', () => {
    window.nodes = [];
    window.networks = {
        "mainnet": true,
        "testnet-10": true,
        "testnet-11": true,
    };

    table = document.createElement('table');
    document.body.appendChild(table);

    thead = document.createElement('thead');
    table.appendChild(thead);
    thead.innerHTML = "<tr><th>SID:UID</th><th>SERVICE</th><th>VERSION</th><th>FQDN</th><th>PROTO</th><th>ENCODING</th><th>NETWORK</th><th>STATUS</th><th class='right'>PEERS</th><th class='right'>CLIENTS/CAP</th><th class='right'>LOAD</th></tr>";

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
            window.nodes = data;
            render();
            setTimeout(fetchData, 5000);
        })
        .catch(error => {
            setTimeout(fetchData, 5000);
            console.error('Error fetching data:', error);
        });
}

function filter(node, ctx) {
    if (!window.networks[node.network]) {
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
        offline : document.getElementById('offline').checked,
        delegators : document.getElementById('delegators').checked,
    };

    let resort = false;
    let tbody = document.getElementById("nodes");

    const status = nodes
        .filter((node) => node.status === 'online')
        .reduce((acc, node) => {
            const network = node.network;
            if (!acc[network]) {
                acc[network] = { clients: 0, capacity: 0 };
            }
            acc[network].clients += node.clients;
            acc[network].capacity += node.capacity;
            return acc;
        }, {});

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
            peers,
            clients,
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

        let load = (clients / capacity * 100.0).toFixed(2);
        let peers_ = peers.toLocaleString();
        let clients_ = clients.toLocaleString();
        let capacity_ = capacity.toLocaleString();
        el.innerHTML = `<td>${sid}:${uid}</td><td>${service}</td><td>${version}</td><td>${fqdn}</td><td>${protocol}</td><td>${encoding}</td><td>${network}</td><td>${status}</td>`;
        if (status != "offline") {
            el.innerHTML += `<td class='wide right'>${peers_}</td><td class='wide right'>${clients_} / ${capacity_}</td><td class='wide right'>${load}%</td>`;
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

            if (window.networks[network] == undefined) {
                window.networks[network] = true;
            }

            document.getElementById(`${network}-filter`).addEventListener('change', () => {
                window.networks[network] = document.getElementById(`${network}-filter`).checked;
                render();
            });
            el = document.getElementById(`${network}-data`);
        }
        let load = (status.clients / status.capacity * 100.0).toFixed(2);
        let clients = status.clients.toLocaleString();
        let capacity = status.capacity.toLocaleString();
        el.innerHTML = `${clients} / ${capacity} &nbsp;${load}%`;
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
