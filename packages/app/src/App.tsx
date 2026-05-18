import { useState } from "react";
import { Button } from "ui";

function App() {
  const [count, setCount] = useState(0);

  return (
    <div className="min-h-screen bg-background text-foreground">
      <Button onClick={() => setCount((c) => c + 1)} size="sm">
        Count is {count}
      </Button>
    </div>
  );
}

export default App;
