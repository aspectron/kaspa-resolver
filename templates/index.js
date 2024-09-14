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
    thead.innerHTML = "<tr><th>SID:UID</th><th>SERVICE</th><th>VERSION</th><th class='fqdn'>FQDN</th><th>PROTO</th><th>ENCODING</th><th>NETWORK</th><th>STATUS</th><th class='right'>PEERS</th><th class='right'>CLIENTS/CAP</th><th class='right'>LOAD</th></tr>";

    tbody = document.createElement('tbody');
    tbody.id = "nodes";
    table.appendChild(tbody);

    ["offline", "delegators"].forEach((id) => {
        document.getElementById(id).addEventListener('change', () => {
            render();
        });
    });

    document.getElementById('fqdn').addEventListener('change', (e) => {
        let checked = e.target.checked;
        if (!e.target.checked) {
            document.getElementById("fqdn-style").innerHTML = ".fqdn { display: none; }";
        } else {
            document.getElementById("fqdn-style").innerHTML = "";
        }

        // Array.from(document.getElementsByClassName('fqdn')).forEach((el) => {
        //     el.classList.toggle('hidden');
        // });
    });

    ["sort-fqdn", "sort-sid", "sort-network"].forEach((id) => {
        document.getElementById(id).addEventListener('change', (e) => {
            window.resolver.sort = id.split('-')[1];
            render();
        });
    });

    document.getElementById(`sort-${window.resolver.sort}`).checked = true;

    fetchData();
});

function fetchData() {
    fetch('/status/json')
        .then(response => response.json())
        .then(data => {
            window.resolver.nodes = data;
            render();
            setTimeout(fetchData, 5000);
        })
        .catch(error => {
            setTimeout(fetchData, 5000);
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

function sortData(node) {
    switch (window.resolver.sort) {
        case "fqdn":
            return node.fqdn;
        case "network":
            return node.network;
        case "sid":
            return node.sid;
        default:
            return node.fqdn;
    }
}

function render() {

    let ctx = {
        offline : document.getElementById('offline').checked,
        delegators : document.getElementById('delegators').checked,
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
            fqdn,
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
            tbody.appendChild(el);
            resort = true;
        }
        el.className = filter(node, ctx);

        if (resort) {
            el.setAttribute('data-sort', sortData(node));
        }

        let load = (clients / capacity * 100.0).toFixed(2);
        let peers_ = peers.toLocaleString();
        let clients_ = clients.toLocaleString();
        let capacity_ = capacity.toLocaleString();
        el.innerHTML = `<td>${sid}:${uid}</td><td>${service}</td><td>${version}</td><td class='fqdn'>${fqdn}</td><td>${protocol}</td><td>${encoding}</td><td>${network}</td><td>${status}</td>`;
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
