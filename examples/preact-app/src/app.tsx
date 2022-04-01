import { useQuery, useSubscription } from "@urql/preact";

const testQuery = `
query {
  hero {
    name
  }
}
`;

function TestQuery() {
  const [result, reexecuteQuery] = useQuery({
    query: testQuery,
  });

  const { data, fetching, error } = result;

  if (fetching) return <p>Loading...</p>;
  if (error) return <p>Oh no... {error.message}</p>;

  return (
    <pre>{JSON.stringify(data, null, 2)}</pre>
  );
}

const newMessages = `
  subscription MessageSub {
    helloWorld
  }
`;

function handleSubscription(messages = [], response) {
  return [response.helloWorld, ...messages];
};

function TestSubscription() {
  const [res] = useSubscription({ query: newMessages }, handleSubscription);

  if (!res.data) {
    return <p>No new messages</p>;
  }

  return (
        <p>
          {res.data}
        </p>
  ); 
}

export function App() {
  return <>
    <TestQuery />
    <TestSubscription />
  </>
}
