import readline from 'readline';

const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout,
  terminal: false,
});

function send(msg) {
  process.stdout.write(JSON.stringify(msg) + '\n');
}

rl.on('line', (line) => {
  const trimmed = line.trim();
  if (!trimmed) return;
  try {
    const req = JSON.parse(trimmed);
    const { id, method, params } = req;

    if (method === 'initialize') {
      send({
        jsonrpc: '2.0',
        id,
        result: {
          protocolVersion: '2024-11-05',
          capabilities: { tools: {} },
          serverInfo: { name: 'mock-test-server', version: '1.0.0' },
        },
      });
    } else if (method === 'notifications/initialized') {
      // Notification, no reply
    } else if (method === 'tools/list') {
      send({
        jsonrpc: '2.0',
        id,
        result: {
          tools: [
            {
              name: 'echo',
              title: 'Echo tool',
              description: 'Echoes back input message',
              inputSchema: {
                type: 'object',
                properties: {
                  message: { type: 'string' },
                },
                required: ['message'],
              },
            },
            {
              name: 'add',
              title: 'Add numbers',
              description: 'Adds two numbers',
              inputSchema: {
                type: 'object',
                properties: {
                  a: { type: 'number' },
                  b: { type: 'number' },
                },
                required: ['a', 'b'],
              },
            },
          ],
        },
      });
    } else if (method === 'tools/call') {
      const toolName = params?.name;
      const args = params?.arguments || {};

      if (toolName === 'echo') {
        send({
          jsonrpc: '2.0',
          id,
          result: {
            content: [{ type: 'text', text: `echo: ${args.message || ''}` }],
            isError: false,
          },
        });
      } else if (toolName === 'add') {
        const sum = (Number(args.a) || 0) + (Number(args.b) || 0);
        send({
          jsonrpc: '2.0',
          id,
          result: {
            content: [{ type: 'text', text: String(sum) }],
            isError: false,
          },
        });
      } else {
        send({
          jsonrpc: '2.0',
          id,
          error: { code: -32601, message: `Tool '${toolName}' not found` },
        });
      }
    } else if (method === 'ping') {
      send({ jsonrpc: '2.0', id, result: {} });
    }
  } catch (err) {
    // Ignore parse errors on bad lines
  }
});
