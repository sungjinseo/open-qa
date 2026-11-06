import React from "react";

export class Counter extends React.Component<{}, { count: number }> {
  state = { count: 0 };
  inc() {
    this.setState({ count: this.state.count + 1 });
  }
  render() {
    return <button onClick={() => this.inc()}>{this.state.count}</button>;
  }
}
