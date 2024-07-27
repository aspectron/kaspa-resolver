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

    const status = nodes
        .filter((node) => node.status === 'online')
        .reduce((acc, node) => {
            const network = node.network;
            if (!acc[network]) {
                acc[network] = { connections: 0, capacity: 0 };
            }
            acc[network].connections += node.connections;
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
        let connections_ = connections.toLocaleString();
        let capacity_ = capacity.toLocaleString();
        el.innerHTML = `<td>${sid}:${uid}</td><td>${service}</td><td>${version}</td><td>${fqdn}</td><td>${protocol}</td><td>${encoding}</td><td>${network}</td><td>${status}</td>`;
        if (status != "offline") {
            el.innerHTML += `<td class='wide right'>${connections_}/${capacity_}</td><td class='wide right'>${load}%</td>`;
        }
    });

    if (resort) {
        sort();
    }

    document.getElementById('status').innerText = Object.entries(status).map(([network, status]) => {
        let load = (status.connections / status.capacity * 100.0).toFixed(2);
        let connections = status.connections.toLocaleString();
        let capacity = status.capacity.toLocaleString();
        return `${network}: ${connections}/${capacity} ${load}%`;
    }).join('   ');
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
