module Fly

  module Math

    extend Component

    ### Basis math methods ###

    # '+': !rb
    #   (n1 n2 -- sum)
    #   drop + drop
    #
    word '+', :add, [Numeric, Numeric] do
      drop + drop
    end

    # '-': !rb
    #   (n1 n2 -- dif)
    #   a,b = drop,drop;
    #   b - a
    #
    word '-', :sub, [Numeric, Numeric] do
      a,b = drop,drop
      b - a
    end

    # '*': !rb
    #   (n1 n2 -- dif)
    #   drop * drop
    #
    word '*', :mul, [Numeric, Numeric] do
      drop * drop
    end

    word '%', :mod, [Numeric, Numeric] do
      a,b = drop,drop
      b % a
    end

    word '**', :pow [Numeric, Numeric] do
       a,b = drop,drop
       b ** a
    end

    word '/', :div, [Numeric, Numeric] do
      a,b = drop,drop
      # defeat Ruby's integer division
      if a.integer? and b.integer? and b % a != 0
        b / a.to_f
      else
        b / a
      end
    end

    ### To int methods ###

    word 'floor', [Numeric] do push drop.floor end
    word 'ceil',  [Numeric] do push drop.ceil end
    word 'round', [Numeric] do push drop.round end

    ### Higher math methods ###

    word 'abs'   , [Numeric]  do push drop.abs end
    word 'sqrt'  , [Numeric]  do push Math.sqrt( drop ) end
    word 'exp'   , [Numeric]  do push Math.exp( drop )   end
    word 'log'   , [Numeric]  do push Math.log( drop )   end
    word 'log10' , [Numeric]  do push Math.log10( drop ) end
    word 'sin'   , [Numeric]  do push Math.sin( drop )   end
    word 'cos'   , [Numeric]  do push Math.cos( drop )   end
    word 'tan'   , [Numeric]  do push Math.tan( drop )   end
    word 'sinh'  , [Numeric]  do push Math.sinh( drop )  end
    word 'cosh'  , [Numeric]  do push Math.cosh( drop )  end
    word 'tanh'  , [Numeric]  do push Math.tanh( drop )  end
    word 'asin'  , [Numeric]  do push Math.asin( drop )  end
    word 'acos'  , [Numeric]  do push Math.acos( drop )  end
    word 'atan'  , [Numeric]  do push Math.atan( drop )  end
    word 'sinh'  , [Numeric]  do push Math.asinh( drop ) end
    word 'acosh' , [Numeric]  do push Math.acosh( drop ) end
    word 'atanh' , [Numeric]  do push Math.atanh( drop ) end

    word 'atan2' , [Numeric, Numeric] do
      a,b = drop,drop
      Math.atan2( b, a )
    end

    ### Bitewise manipulation methods ###

    # Warning:  These methods convert their operands to ints before operation

    word '~', [Numeric] do
      ~drop.to_i
    end

    word '&', [Numeric, Numeric] do
      a,b = drop,drop
      b.to_i & a.to_i
    end

    word '|', [Numeric, Numeric] do
      a,b = drop,drop
      b.to_i | a.to_i
    end

    word '^', [Numeric, Numeric] do
      a,b = drop,drop
      b.to_i ^ a.to_i
    end

    ### Modes ###

    word 'bin' do mode = :bin end
    word 'sen' do mode = :sen end
    word 'oct' do mode = :oct end
    word 'dec' do mode = :dec end
    word 'hex' do mode = :hex end

    private



  end

end
